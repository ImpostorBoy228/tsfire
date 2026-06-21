use std::sync::OnceLock;
use std::iter;

use euclid::Size2D;
use euclid::Scale;
use app_units::Au;
use style_traits::CSSPixel;

use style::dom::{
    TElement, TNode, TDocument, TShadowRoot, NodeInfo, LayoutIterator,
    OpaqueNode, AttributeProvider,
};
use style::data::{ElementDataRef, ElementDataMut};
use style::selector_parser::{
    SelectorImpl, PseudoElement, NonTSPseudoClass, Lang, AttrValue,
};
use style::properties::{ComputedValues, PropertyDeclarationBlock, parse_style_attribute};
use style::properties::style_structs::Font;
use style::stylist::Stylist;
use style::shared_lock::{Locked, StylesheetGuards, SharedRwLock};
use style::context::SharedStyleContext;
use style::values::AtomIdent;
use style::values::computed::Display;
use style::applicable_declarations::ApplicableDeclarationBlock;
use style::device::{Device, servo::FontMetricsProvider};
use style::media_queries::MediaType;
use style::queries::values::PrefersColorScheme;
use style::font_metrics::FontMetrics;
use style::values::computed::{CSSPixelLength, Length, LengthPercentage, NonNegativeLengthPercentage};
use style::values::computed::{BorderStyle, Overflow, Float, Clear, TextAlign};
use style::values::computed::image::Image as StyloImage;
use style::values::computed::position::Inset;
use style::values::computed::text::TextDecorationLine as StyloTextDecorationLine;
use style::values::generics::image::GradientItem;
use style::values::generics::position::GenericInset;
use style::values::generics::length::GenericSize;
use style::values::specified::font::QueryFontMetricsFlags;
use style::values::computed::font::GenericFontFamily;
use style::servo::media_features::PointerCapabilities;
use style::stylesheets::UrlExtraData;
use style::stylesheets::CssRuleType;

use selectors::matching::{ElementSelectorFlags, VisitedHandlingMode, QuirksMode};
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::sink::Push;
use selectors::bloom::BloomFilter;
use selectors::Element as SelectorsElement;
use selectors::OpaqueElement;
use selectors::parser::SelectorImpl as SelectorImplTrait;

use servo_arc::{Arc, ArcBorrow};
use dom::ElementState;

fn empty_web_local_name() -> &'static web_atoms::LocalName {
    static EMPTY: OnceLock<web_atoms::LocalName> = OnceLock::new();
    EMPTY.get_or_init(|| web_atoms::LocalName::default())
}

fn empty_web_namespace() -> &'static web_atoms::Namespace {
    static EMPTY: OnceLock<web_atoms::Namespace> = OnceLock::new();
    EMPTY.get_or_init(|| web_atoms::Namespace::default())
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PhantomElement;

impl NodeInfo for PhantomElement {
    fn is_element(&self) -> bool { true }
    fn is_text_node(&self) -> bool { false }
}

impl AttributeProvider for PhantomElement {
    fn get_attr(&self, _attr: &style::LocalName, _namespace: &style::Namespace) -> Option<String> {
        None
    }
}

impl SelectorsElement for PhantomElement {
    type Impl = SelectorImpl;

    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::from_non_null_ptr(std::ptr::NonNull::dangling())
    }

    fn parent_element(&self) -> Option<Self> { None }

    fn parent_node_is_shadow_root(&self) -> bool { false }

    fn containing_shadow_host(&self) -> Option<Self> { None }

    fn is_pseudo_element(&self) -> bool { false }

    fn prev_sibling_element(&self) -> Option<Self> { None }

    fn next_sibling_element(&self) -> Option<Self> { None }

    fn first_element_child(&self) -> Option<Self> { None }

    fn is_html_element_in_html_document(&self) -> bool { false }

    fn has_local_name(&self, _: &<SelectorImpl as SelectorImplTrait>::BorrowedLocalName) -> bool {
        false
    }

    fn has_namespace(&self, _: &<SelectorImpl as SelectorImplTrait>::BorrowedNamespaceUrl) -> bool {
        false
    }

    fn is_same_type(&self, _: &Self) -> bool { false }

    fn attr_matches(
        &self,
        _: &NamespaceConstraint<&<SelectorImpl as SelectorImplTrait>::NamespaceUrl>,
        _: &<SelectorImpl as SelectorImplTrait>::LocalName,
        _: &AttrSelectorOperation<&<SelectorImpl as SelectorImplTrait>::AttrValue>,
    ) -> bool {
        false
    }

    fn match_non_ts_pseudo_class(
        &self,
        _: &NonTSPseudoClass,
        _: &mut selectors::matching::MatchingContext<SelectorImpl>,
    ) -> bool {
        false
    }

    fn match_pseudo_element(
        &self,
        _: &PseudoElement,
        _: &mut selectors::matching::MatchingContext<SelectorImpl>,
    ) -> bool {
        false
    }

    fn apply_selector_flags(&self, _: ElementSelectorFlags) {}

    fn is_link(&self) -> bool { false }

    fn is_html_slot_element(&self) -> bool { false }

    fn has_id(&self, _: &<SelectorImpl as SelectorImplTrait>::Identifier, _: CaseSensitivity) -> bool {
        false
    }

    fn has_class(&self, _: &<SelectorImpl as SelectorImplTrait>::Identifier, _: CaseSensitivity) -> bool {
        false
    }

    fn has_custom_state(&self, _: &<SelectorImpl as SelectorImplTrait>::Identifier) -> bool { false }

    fn imported_part(
        &self,
        _: &<SelectorImpl as SelectorImplTrait>::Identifier,
    ) -> Option<<SelectorImpl as SelectorImplTrait>::Identifier> {
        None
    }

    fn is_part(&self, _: &<SelectorImpl as SelectorImplTrait>::Identifier) -> bool { false }

    fn is_empty(&self) -> bool { true }

    fn is_root(&self) -> bool { false }

    fn add_element_unique_hashes(&self, _: &mut BloomFilter) -> bool { false }
}

impl TElement for PhantomElement {
    type ConcreteNode = PhantomNode;
    type TraversalChildrenIterator = iter::Empty<PhantomNode>;

    fn as_node(&self) -> PhantomNode { PhantomNode }

    fn traversal_children(&self) -> LayoutIterator<Self::TraversalChildrenIterator> {
        LayoutIterator(iter::empty())
    }

    fn is_html_element(&self) -> bool { false }
    fn is_mathml_element(&self) -> bool { false }
    fn is_svg_element(&self) -> bool { false }

    fn style_attribute(&self) -> Option<ArcBorrow<'_, Locked<PropertyDeclarationBlock>>> {
        None
    }

    fn animation_rule(
        &self,
        _: &SharedStyleContext,
    ) -> Option<Arc<Locked<PropertyDeclarationBlock>>> {
        None
    }

    fn transition_rule(
        &self,
        _: &SharedStyleContext,
    ) -> Option<Arc<Locked<PropertyDeclarationBlock>>> {
        None
    }

    fn state(&self) -> ElementState { ElementState::empty() }

    fn has_part_attr(&self) -> bool { false }
    fn exports_any_part(&self) -> bool { false }

    fn id(&self) -> Option<&stylo_atoms::Atom> { None }

    fn each_class<F>(&self, _: F) where F: FnMut(&AtomIdent) {}

    fn each_custom_state<F>(&self, _: F) where F: FnMut(&AtomIdent) {}

    fn each_attr_name<F>(&self, _: F) where F: FnMut(&style::LocalName) {}

    fn has_dirty_descendants(&self) -> bool { false }
    fn has_snapshot(&self) -> bool { false }
    fn handled_snapshot(&self) -> bool { true }

    unsafe fn set_handled_snapshot(&self) {}
    unsafe fn set_dirty_descendants(&self) {}
    unsafe fn unset_dirty_descendants(&self) {}

    fn store_children_to_process(&self, _: isize) {}
    fn did_process_child(&self) -> isize { 0 }

    unsafe fn ensure_data(&self) -> ElementDataMut<'_> {
        panic!("PhantomElement::ensure_data not supported")
    }

    unsafe fn clear_data(&self) {}

    fn has_data(&self) -> bool { false }

    fn borrow_data(&self) -> Option<ElementDataRef<'_>> { None }

    fn mutate_data(&self) -> Option<ElementDataMut<'_>> { None }

    fn skip_item_display_fixup(&self) -> bool { true }

    fn may_have_animations(&self) -> bool { false }

    fn has_animations(&self, _: &SharedStyleContext) -> bool { false }

    fn has_css_animations(
        &self,
        _: &SharedStyleContext,
        _: Option<PseudoElement>,
    ) -> bool { false }

    fn has_css_transitions(
        &self,
        _: &SharedStyleContext,
        _: Option<PseudoElement>,
    ) -> bool { false }

    fn shadow_root(&self) -> Option<PhantomShadowRoot> { None }

    fn containing_shadow(&self) -> Option<PhantomShadowRoot> { None }

    fn lang_attr(&self) -> Option<AttrValue> { None }

    fn match_element_lang(&self, _: Option<Option<AttrValue>>, _: &Lang) -> bool { false }

    fn is_html_document_body_element(&self) -> bool { false }

    fn synthesize_presentational_hints_for_legacy_attributes<V>(
        &self,
        _: VisitedHandlingMode,
        _: &mut V,
    ) where V: Push<ApplicableDeclarationBlock> {}

    fn local_name(&self) -> &<SelectorImpl as SelectorImplTrait>::BorrowedLocalName {
        empty_web_local_name()
    }

    fn namespace(&self) -> &<SelectorImpl as SelectorImplTrait>::BorrowedNamespaceUrl {
        empty_web_namespace()
    }

    fn query_container_size(
        &self,
        _: &Display,
    ) -> euclid::default::Size2D<Option<Au>> {
        euclid::default::Size2D::new(None, None)
    }

    fn has_selector_flags(&self, _: ElementSelectorFlags) -> bool { false }

    fn relative_selector_search_direction(&self) -> ElementSelectorFlags {
        ElementSelectorFlags::empty()
    }
}

// --- Phantom Node, Document, ShadowRoot ---

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PhantomNode;

impl NodeInfo for PhantomNode {
    fn is_element(&self) -> bool { true }
    fn is_text_node(&self) -> bool { false }
}

impl TNode for PhantomNode {
    type ConcreteElement = PhantomElement;
    type ConcreteDocument = PhantomDocument;
    type ConcreteShadowRoot = PhantomShadowRoot;

    fn parent_node(&self) -> Option<Self> { None }
    fn first_child(&self) -> Option<Self> { None }
    fn last_child(&self) -> Option<Self> { None }
    fn prev_sibling(&self) -> Option<Self> { None }
    fn next_sibling(&self) -> Option<Self> { None }

    fn owner_doc(&self) -> PhantomDocument { PhantomDocument }

    fn is_in_document(&self) -> bool { true }

    fn traversal_parent(&self) -> Option<PhantomElement> { None }

    fn opaque(&self) -> OpaqueNode { OpaqueNode(0) }

    fn debug_id(self) -> usize { 0 }

    fn as_element(&self) -> Option<PhantomElement> { Some(PhantomElement) }

    fn as_document(&self) -> Option<PhantomDocument> { Some(PhantomDocument) }

    fn as_shadow_root(&self) -> Option<PhantomShadowRoot> { None }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PhantomDocument;

impl TDocument for PhantomDocument {
    type ConcreteNode = PhantomNode;

    fn as_node(&self) -> PhantomNode { PhantomNode }

    fn is_html_document(&self) -> bool { true }

    fn quirks_mode(&self) -> QuirksMode { QuirksMode::NoQuirks }

    fn shared_lock(&self) -> &SharedRwLock {
        static LOCK: OnceLock<SharedRwLock> = OnceLock::new();
        LOCK.get_or_init(|| SharedRwLock::new_leaked())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PhantomShadowRoot;

impl TShadowRoot for PhantomShadowRoot {
    type ConcreteNode = PhantomNode;

    fn as_node(&self) -> PhantomNode { PhantomNode }

    fn host(&self) -> PhantomElement { PhantomElement }

    fn style_data<'a>(&self) -> Option<&'a style::stylist::CascadeData>
    where Self: 'a { None }
}

// --- Mock FontMetricsProvider ---

#[derive(Debug)]
struct MockFontMetrics;

unsafe impl Send for MockFontMetrics {}
unsafe impl Sync for MockFontMetrics {}

impl style::device::servo::FontMetricsProvider for MockFontMetrics {
    fn query_font_metrics(
        &self,
        _vertical: bool,
        _font: &Font,
        _base_size: CSSPixelLength,
        _flags: QueryFontMetricsFlags,
    ) -> FontMetrics {
        FontMetrics::default()
    }

    fn base_size_for_generic(&self, _generic: GenericFontFamily) -> Length {
        Length::new(16.0)
    }
}

fn make_initial_font() -> Font {
    use style::properties::longhands::*;
    Font {
        _x_lang: _x_lang::get_initial_value(),
        font_family: font_family::get_initial_value(),
        font_feature_settings: font_feature_settings::get_initial_value(),
        font_language_override: font_language_override::get_initial_value(),
        font_size: font_size::get_initial_value(),
        font_stretch: font_stretch::get_initial_value(),
        font_style: font_style::get_initial_value(),
        font_synthesis_weight: font_synthesis_weight::get_initial_value(),
        font_variant_east_asian: font_variant_east_asian::get_initial_value(),
        font_variant_ligatures: font_variant_ligatures::get_initial_value(),
        font_variant_numeric: font_variant_numeric::get_initial_value(),
        font_variation_settings: font_variation_settings::get_initial_value(),
        font_weight: font_weight::get_initial_value(),
        line_height: line_height::get_initial_value(),
        font_variant_caps: font_variant_caps::get_initial_value(),
        font_kerning: font_kerning::get_initial_value(),
        font_variant_position: font_variant_position::get_initial_value(),
        font_optical_sizing: font_optical_sizing::get_initial_value(),
        hash: 0,
    }
}

// --- Global Stylist ---

static GUARD_LOCK: OnceLock<SharedRwLock> = OnceLock::new();

pub fn create_global_stylist() -> Stylist {
    let viewport = Size2D::<f32, CSSPixel>::new(1024.0, 768.0);
    
    let provider: Box<dyn FontMetricsProvider> = Box::new(MockFontMetrics);
    
    let device = Device::new(
        MediaType::screen(),
        QuirksMode::NoQuirks,
        viewport,
        Scale::new(1.0),
        provider,
        ComputedValues::initial_values_with_font_override(make_initial_font()),
        PrefersColorScheme::Light,
        PointerCapabilities::empty(),
        PointerCapabilities::empty(),
    );
    
    Stylist::new(device, QuirksMode::NoQuirks)
}

// --- Helper functions ---

pub fn compute_style(
    stylist: &Stylist,
    parent_style: Option<&ComputedValues>,
    declarations: Arc<Locked<PropertyDeclarationBlock>>,
) -> Arc<ComputedValues> {
    let lock = GUARD_LOCK.get_or_init(|| SharedRwLock::new_leaked());
    let guard = lock.read();
    let guards = StylesheetGuards::same(&guard);
    let parent = parent_style.unwrap_or_else(|| stylist.device().default_computed_values());
    stylist.compute_for_declarations::<PhantomElement>(&guards, parent, declarations)
}

// --- Bridge: Stylo → custom style.rs ComputedValues ---

/// Parse CSS declaration text into Stylo's PropertyDeclarationBlock
pub fn parse_css_declarations(css: &str) -> PropertyDeclarationBlock {
    let url = url::Url::parse("about:blank").unwrap();
    let url_data = UrlExtraData::from(url);
    parse_style_attribute(
        css,
        &url_data,
        None,
        QuirksMode::NoQuirks,
        CssRuleType::Style,
    )
}

/// Convert matching CssRules into a single CSS declaration string
pub fn matched_declarations_to_css(rules: &[crate::parsing::CssRule], element: &crate::parsing::DomElement) -> String {
    use crate::parsing::rule_matches_element;
    let mut css = String::new();
    let mut matched: Vec<(&crate::parsing::CssRule, u32)> = Vec::new();
    for rule in rules {
        if rule_matches_element(rule, element) {
            let spec = rule.selectors.iter()
                .map(|s| {
                    let saf: u32 = s.specificity().into();
                    saf
                })
                .max()
                .unwrap_or(0);
            matched.push((rule, spec));
        }
    }
    matched.sort_by_key(|(_, spec)| *spec);
    for (rule, _) in &matched {
        for (prop, val) in &rule.declarations {
            css.push_str(prop);
            css.push(':');
            css.push_str(val);
            css.push(';');
        }
    }
    css
}

fn stylo_color_to_our(color: &style::color::AbsoluteColor) -> crate::parsing::Color {
    use style::color::ColorSpace;
    let srgb = color.to_color_space(ColorSpace::Srgb);
    crate::parsing::Color(
        (srgb.components.0 * 255.0) as u8,
        (srgb.components.1 * 255.0) as u8,
        (srgb.components.2 * 255.0) as u8,
        (srgb.alpha * 255.0) as u8,
    )
}

fn resolve_stylo_color(stylo: &ComputedValues, color: &style::values::computed::Color) -> crate::parsing::Color {
    stylo_color_to_our(&stylo.resolve_color(color))
}

fn to_our_length_from_lp(lp: &LengthPercentage) -> crate::parsing::Length {
    lp.to_length().map(|l| crate::parsing::Length::Px(l.px())).unwrap_or(crate::parsing::Length::Auto)
}

fn to_our_length_from_nnlp(lp: &NonNegativeLengthPercentage) -> crate::parsing::Length {
    lp.0.to_length().map(|l| crate::parsing::Length::Px(l.px())).unwrap_or(crate::parsing::Length::Auto)
}

fn au_to_px(au: &app_units::Au) -> f32 {
    au.to_f32_px()
}

fn to_our_border_style(s: BorderStyle) -> crate::parsing::BorderStyle {
    match s {
        BorderStyle::None => crate::parsing::BorderStyle::None,
        BorderStyle::Hidden => crate::parsing::BorderStyle::Hidden,
        BorderStyle::Solid => crate::parsing::BorderStyle::Solid,
        BorderStyle::Dashed => crate::parsing::BorderStyle::Dashed,
        BorderStyle::Dotted => crate::parsing::BorderStyle::Dotted,
        BorderStyle::Double => crate::parsing::BorderStyle::Double,
        BorderStyle::Groove => crate::parsing::BorderStyle::Groove,
        BorderStyle::Ridge => crate::parsing::BorderStyle::Ridge,
        BorderStyle::Inset => crate::parsing::BorderStyle::Inset,
        BorderStyle::Outset => crate::parsing::BorderStyle::Outset,
    }
}

fn to_our_overflow(o: Overflow) -> crate::parsing::Overflow {
    match o {
        Overflow::Visible => crate::parsing::Overflow::Visible,
        Overflow::Hidden => crate::parsing::Overflow::Hidden,
        Overflow::Scroll => crate::parsing::Overflow::Scroll,
        Overflow::Auto => crate::parsing::Overflow::Auto,
        Overflow::Clip => crate::parsing::Overflow::Clip,
    }
}

fn to_our_float(f: Float) -> crate::parsing::Float {
    match f {
        Float::None => crate::parsing::Float::None,
        Float::Left => crate::parsing::Float::Left,
        Float::Right => crate::parsing::Float::Right,
        Float::InlineStart => crate::parsing::Float::InlineStart,
        Float::InlineEnd => crate::parsing::Float::InlineEnd,
    }
}

fn to_our_clear(c: Clear) -> crate::parsing::Clear {
    match c {
        Clear::None => crate::parsing::Clear::None,
        Clear::Left => crate::parsing::Clear::Left,
        Clear::Right => crate::parsing::Clear::Right,
        Clear::Both => crate::parsing::Clear::Both,
        Clear::InlineStart => crate::parsing::Clear::InlineStart,
        Clear::InlineEnd => crate::parsing::Clear::InlineEnd,
    }
}

fn to_our_text_align(ta: TextAlign) -> crate::parsing::TextAlign {
    match ta {
        TextAlign::Left => crate::parsing::TextAlign::Left,
        TextAlign::Right => crate::parsing::TextAlign::Right,
        TextAlign::Center => crate::parsing::TextAlign::Center,
        TextAlign::Justify => crate::parsing::TextAlign::Justify,
        TextAlign::Start => crate::parsing::TextAlign::Start,
        TextAlign::End => crate::parsing::TextAlign::End,
        _ => crate::parsing::TextAlign::Start,
    }
}

fn to_our_inset(inset: &Inset) -> crate::parsing::Length {
    match inset {
        GenericInset::LengthPercentage(lp) => to_our_length_from_lp(lp),
        _ => crate::parsing::Length::Auto,
    }
}

fn extract_background_images(bg: &style::properties::style_structs::Background) -> Vec<crate::parsing::BackgroundImage> {
    use style::values::computed::image::Gradient as ComputedGradient;
    let images = bg.background_image.0.iter();
    let mut result = Vec::new();
    for img in images {
        match img {
            StyloImage::Gradient(grad) => {
                let items = match &**grad {
                    ComputedGradient::Linear { items, .. } => items,
                    ComputedGradient::Radial { items, .. } => items,
                    _ => continue,
                };
                let colors: Vec<&style::values::computed::Color> = items.iter().filter_map(|item| {
                    match item {
                        GradientItem::SimpleColorStop(c) => Some(c),
                        GradientItem::ComplexColorStop { color, position: _ } => Some(color),
                        _ => None,
                    }
                }).collect();
                if colors.len() >= 2 {
                    let from = resolve_stylo_color_from_abs(colors[0]);
                    let to = resolve_stylo_color_from_abs(colors[colors.len() - 1]);
                    result.push(crate::parsing::BackgroundImage::Gradient { from, to, vertical: true });
                }
            }
            _ => {}
        }
    }
    result
}

fn resolve_stylo_color_from_abs(color: &style::values::computed::Color) -> crate::parsing::Color {
    use style::color::ColorSpace;
    let abs = color.resolve_to_absolute(&style::color::AbsoluteColor::BLACK);
    let srgb = abs.to_color_space(ColorSpace::Srgb);
    crate::parsing::Color(
        (srgb.components.0 * 255.0) as u8,
        (srgb.components.1 * 255.0) as u8,
        (srgb.components.2 * 255.0) as u8,
        (srgb.alpha * 255.0) as u8,
    )
}

/// Convert Stylo's ComputedValues to our custom format
pub fn convert_stylo_computed_values(stylo: &ComputedValues) -> crate::parsing::ComputedValues {
    use crate::parsing::{ComputedValues as OurCV, Display, Length, Position, BoxShadow, TextDecorationLine, VerticalAlign, WhiteSpace, BoxSizing, Visibility, Direction};

    let box_ = stylo.get_box();

    let display = match box_.display {
        style::values::computed::Display::None => Display::None,
        style::values::computed::Display::Inline => Display::Inline,
        style::values::computed::Display::InlineBlock => Display::InlineBlock,
        style::values::computed::Display::Flex => Display::Flex,
        style::values::computed::Display::InlineFlex => Display::InlineFlex,
        style::values::computed::Display::Grid => Display::Grid,
        style::values::computed::Display::InlineGrid => Display::InlineGrid,
        style::values::computed::Display::Table => Display::Table,
        style::values::computed::Display::InlineTable => Display::InlineTable,
        style::values::computed::Display::TableRowGroup => Display::TableRowGroup,
        style::values::computed::Display::TableHeaderGroup => Display::TableHeaderGroup,
        style::values::computed::Display::TableFooterGroup => Display::TableFooterGroup,
        style::values::computed::Display::TableRow => Display::TableRow,
        style::values::computed::Display::TableColumnGroup => Display::TableColumnGroup,
        style::values::computed::Display::TableColumn => Display::TableColumn,
        style::values::computed::Display::TableCell => Display::TableCell,
        style::values::computed::Display::TableCaption => Display::TableCaption,
        _ => Display::Block,
    };

    let position = match box_.position {
        style::values::computed::PositionProperty::Absolute => Position::Absolute,
        style::values::computed::PositionProperty::Fixed => Position::Fixed,
        style::values::computed::PositionProperty::Relative => Position::Relative,
        style::values::computed::PositionProperty::Sticky => Position::Sticky,
        _ => Position::Static,
    };

    let position_ = stylo.get_position();
    let margin_ = stylo.get_margin();
    let padding_ = stylo.get_padding();
    let inherited_text = stylo.get_inherited_text();
    let background = stylo.get_background();
    let border_ = stylo.get_border();
    let effects_ = stylo.get_effects();
    let text_ = stylo.get_text();
    let outline_ = stylo.get_outline();
    let font_ = stylo.get_font();

    let width = match &position_.width {
        GenericSize::LengthPercentage(lp) => to_our_length_from_lp(&lp.0),
        _ => Length::Auto,
    };

    let height = match &position_.height {
        GenericSize::LengthPercentage(lp) => to_our_length_from_lp(&lp.0),
        _ => Length::Auto,
    };

    let to_margin = |m: &style::values::generics::length::GenericMargin<LengthPercentage>| -> Length {
        match m {
            style::values::generics::length::GenericMargin::LengthPercentage(lp) => {
                to_our_length_from_lp(lp)
            }
            _ => Length::Auto,
        }
    };

    let color = stylo_color_to_our(&inherited_text.color);
    let bg = resolve_stylo_color_from_abs(&background.background_color);
    let font_size = font_.font_size.computed_size.0.px();
    let z_index = match &position_.z_index {
        style::values::generics::position::GenericZIndex::Integer(i) => *i,
        _ => 0,
    };

    // Border
    let border_top_width = au_to_px(&border_.clone_border_top_width().0);
    let border_right_width = au_to_px(&border_.clone_border_right_width().0);
    let border_bottom_width = au_to_px(&border_.clone_border_bottom_width().0);
    let border_left_width = au_to_px(&border_.clone_border_left_width().0);

    let border_top_style = to_our_border_style(border_.clone_border_top_style());
    let border_right_style = to_our_border_style(border_.clone_border_right_style());
    let border_bottom_style = to_our_border_style(border_.clone_border_bottom_style());
    let border_left_style = to_our_border_style(border_.clone_border_left_style());

    let border_top_color = resolve_stylo_color(stylo, &border_.clone_border_top_color());
    let border_right_color = resolve_stylo_color(stylo, &border_.clone_border_right_color());
    let border_bottom_color = resolve_stylo_color(stylo, &border_.clone_border_bottom_color());
    let border_left_color = resolve_stylo_color(stylo, &border_.clone_border_left_color());

    let border_radius_ = |r: &style::values::computed::BorderCornerRadius| -> f32 {
        r.0.width.0.to_length().map(|l| l.px()).unwrap_or(0.0)
    };
    let border_top_left_radius = border_radius_(&border_.clone_border_top_left_radius());
    let border_top_right_radius = border_radius_(&border_.clone_border_top_right_radius());
    let border_bottom_right_radius = border_radius_(&border_.clone_border_bottom_right_radius());
    let border_bottom_left_radius = border_radius_(&border_.clone_border_bottom_left_radius());

    // Effects
    let opacity = effects_.clone_opacity();
    let box_shadow: Vec<BoxShadow> = effects_.clone_box_shadow().0.iter().map(|bs| {
        BoxShadow {
            offset_x: bs.base.horizontal.px(),
            offset_y: bs.base.vertical.px(),
            blur: bs.base.blur.0.px(),
            spread: bs.spread.px(),
            color: resolve_stylo_color(stylo, &bs.base.color),
            inset: bs.inset,
        }
    }).collect();

    // Overflow
    let overflow_x = to_our_overflow(box_.clone_overflow_x());
    let overflow_y = to_our_overflow(box_.clone_overflow_y());

    // Float / Clear
    let float = to_our_float(box_.clone_float());
    let clear = to_our_clear(box_.clone_clear());

    // Position offsets
    let top = to_our_inset(&position_.top);
    let right = to_our_inset(&position_.right);
    let bottom = to_our_inset(&position_.bottom);
    let left = to_our_inset(&position_.left);

    // Background image
    let background_image = extract_background_images(background);

    // Text decoration
    let td_line = text_.clone_text_decoration_line();
    let text_decoration_line = TextDecorationLine {
        underline: td_line.contains(StyloTextDecorationLine::UNDERLINE),
        overline: td_line.contains(StyloTextDecorationLine::OVERLINE),
        line_through: td_line.contains(StyloTextDecorationLine::LINE_THROUGH),
        blink: td_line.contains(StyloTextDecorationLine::BLINK),
    };
    let text_decoration_color = resolve_stylo_color(stylo, &text_.clone_text_decoration_color());
    let td_style = text_.clone_text_decoration_style();
    let text_decoration_style = match td_style {
        style::properties::longhands::text_decoration_style::computed_value::T::Solid => crate::parsing::BorderStyle::Solid,
        style::properties::longhands::text_decoration_style::computed_value::T::Double => crate::parsing::BorderStyle::Double,
        style::properties::longhands::text_decoration_style::computed_value::T::Dotted => crate::parsing::BorderStyle::Dotted,
        style::properties::longhands::text_decoration_style::computed_value::T::Dashed => crate::parsing::BorderStyle::Dashed,
        style::properties::longhands::text_decoration_style::computed_value::T::Wavy => crate::parsing::BorderStyle::Solid,
        _ => crate::parsing::BorderStyle::None,
    };

    // Text align
    let text_align = to_our_text_align(inherited_text.clone_text_align());

    // Vertical align — default baseline for now
    let vertical_align = VerticalAlign::Baseline;

    // Outline
    let outline_width = au_to_px(&outline_.clone_outline_width().0);
    let outline_style = match outline_.clone_outline_style() {
        style::values::computed::OutlineStyle::Auto => crate::parsing::BorderStyle::None,
        style::values::computed::OutlineStyle::BorderStyle(s) => to_our_border_style(s),
    };
    let outline_color = resolve_stylo_color(stylo, &outline_.clone_outline_color());

    OurCV {
        display,
        position,
        width,
        height,
        margin_top: to_margin(&margin_.margin_top),
        margin_right: to_margin(&margin_.margin_right),
        margin_bottom: to_margin(&margin_.margin_bottom),
        margin_left: to_margin(&margin_.margin_left),
        padding_top: to_our_length_from_nnlp(&padding_.padding_top),
        padding_right: to_our_length_from_nnlp(&padding_.padding_right),
        padding_bottom: to_our_length_from_nnlp(&padding_.padding_bottom),
        padding_left: to_our_length_from_nnlp(&padding_.padding_left),
        color,
        background_color: bg,
        font_size,
        z_index,

        border_top_width,
        border_right_width,
        border_bottom_width,
        border_left_width,
        border_top_style,
        border_right_style,
        border_bottom_style,
        border_left_style,
        border_top_color,
        border_right_color,
        border_bottom_color,
        border_left_color,
        border_top_left_radius,
        border_top_right_radius,
        border_bottom_right_radius,
        border_bottom_left_radius,

        opacity,
        box_shadow,

        overflow_x,
        overflow_y,
        float,
        clear,
        top,
        right,
        bottom,
        left,

        background_image,

        text_decoration_line,
        text_decoration_color,
        text_decoration_style,
        text_align,
        vertical_align,

        // Font & Text (bridge added later)
        line_height: font_size * 1.2,
        font_family: String::from("sans-serif"),
        white_space: WhiteSpace::Normal,
        direction: Direction::Ltr,

        // Sizing
        min_width: Length::Auto,
        max_width: Length::Auto,
        min_height: Length::Auto,
        max_height: Length::Auto,
        box_sizing: BoxSizing::ContentBox,

        visibility: Visibility::Visible,

        outline_width,
        outline_style,
        outline_color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stylo_cv(css: &str) -> ComputedValues {
        let stylist = create_global_stylist();
        let pdb = parse_css_declarations(css);
        let lock = GUARD_LOCK.get_or_init(|| SharedRwLock::new_leaked());
        let wrapped = Arc::new(lock.wrap(pdb));
        let guard = lock.read();
        let guards = StylesheetGuards::same(&guard);
        let arc = stylist.compute_for_declarations::<PhantomElement>(
            &guards,
            stylist.device().default_computed_values(),
            wrapped,
        );
        (*arc).clone()
    }

    fn extract(css: &str) -> crate::parsing::ComputedValues {
        convert_stylo_computed_values(&make_stylo_cv(css))
    }

    #[test]
    fn test_border_widths() {
        let cv = extract("border-top-width:5px;border-right-width:10px;border-bottom-width:15px;border-left-width:20px");
        assert_eq!(cv.border_top_width, 5.0);
        assert_eq!(cv.border_right_width, 10.0);
        assert_eq!(cv.border_bottom_width, 15.0);
        assert_eq!(cv.border_left_width, 20.0);
    }

    #[test]
    fn test_border_styles() {
        let cv = extract("border-top-style:dashed;border-right-style:dotted;border-bottom-style:double;border-left-style:groove");
        assert_eq!(cv.border_top_style, crate::parsing::BorderStyle::Dashed);
        assert_eq!(cv.border_right_style, crate::parsing::BorderStyle::Dotted);
        assert_eq!(cv.border_bottom_style, crate::parsing::BorderStyle::Double);
        assert_eq!(cv.border_left_style, crate::parsing::BorderStyle::Groove);
    }

    #[test]
    fn test_opacity() {
        let cv = extract("opacity:0.42");
        assert!((cv.opacity - 0.42).abs() < 0.001);
    }

    #[test]
    fn test_overflow() {
        let cv = extract("overflow:hidden");
        assert_eq!(cv.overflow_x, crate::parsing::Overflow::Hidden);
        assert_eq!(cv.overflow_y, crate::parsing::Overflow::Hidden);
    }

    #[test]
    fn test_overflow_individual() {
        let cv = extract("overflow-x:scroll;overflow-y:hidden");
        assert_eq!(cv.overflow_x, crate::parsing::Overflow::Scroll);
        assert_eq!(cv.overflow_y, crate::parsing::Overflow::Hidden);
    }

    #[test]
    fn test_float_clear() {
        let cv = extract("float:left;clear:both");
        assert_eq!(cv.float, crate::parsing::Float::Left);
        assert_eq!(cv.clear, crate::parsing::Clear::Both);
    }

    #[test]
    fn test_position_offsets() {
        let cv = extract("top:10px;right:20px;bottom:30px;left:40px");
        assert_eq!(cv.top, crate::parsing::Length::Px(10.0));
        assert_eq!(cv.right, crate::parsing::Length::Px(20.0));
        assert_eq!(cv.bottom, crate::parsing::Length::Px(30.0));
        assert_eq!(cv.left, crate::parsing::Length::Px(40.0));
    }

    #[test]
    fn test_text_decoration() {
        let cv = extract("text-decoration:underline;text-decoration-color:red");
        assert!(cv.text_decoration_line.underline);
        assert_eq!(cv.text_decoration_color, crate::parsing::Color(255, 0, 0, 255));
    }

    #[test]
    fn test_outline() {
        let cv = extract("outline-width:3px;outline-style:solid;outline-color:#ff0000");
        assert_eq!(cv.outline_width, 3.0);
        assert_eq!(cv.outline_style, crate::parsing::BorderStyle::Solid);
        assert_eq!(cv.outline_color, crate::parsing::Color(255, 0, 0, 255));
    }

    #[test]
    fn test_z_index() {
        let cv = extract("z-index:42");
        assert_eq!(cv.z_index, 42);
    }

    #[test]
    fn test_display_none() {
        let cv = extract("display:none");
        assert_eq!(cv.display, crate::parsing::Display::None);
    }

    #[test]
    fn test_display_inline() {
        let cv = extract("display:inline");
        assert_eq!(cv.display, crate::parsing::Display::Inline);
    }
}

/// Compute style for an element using Stylo, returning our custom format
pub fn compute_style_bridge(
    stylist: &Stylist,
    rules: &[crate::parsing::CssRule],
    element: &crate::parsing::DomElement,
) -> crate::parsing::ComputedValues {
    let css = matched_declarations_to_css(rules, element);
    let pdb = parse_css_declarations(&css);
    let lock = GUARD_LOCK.get_or_init(|| SharedRwLock::new_leaked());
    let wrapped = Arc::new(lock.wrap(pdb));
    let stylo_cv = compute_style(stylist, None, wrapped);
    convert_stylo_computed_values(&stylo_cv)
}
