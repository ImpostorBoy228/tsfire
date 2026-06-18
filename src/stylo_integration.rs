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
use style::values::computed::{CSSPixelLength, Length};
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
    let guard = SharedRwLock::new_leaked();
    
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
