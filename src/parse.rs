use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, RcDom, NodeData};

use selectors::Element as SelectorElement;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::matching::{MatchingContext, MatchingMode, matches_selector, NeedsSelectorFlags, MatchingForInvalidation};
use selectors::parser::{SelectorImpl, Selector, SelectorList, ParseRelative, PseudoElement, NonTSPseudoClass};
use selectors::parser::SelectorParseErrorKind;
use selectors::context::{SelectorCaches, QuirksMode};
use selectors::OpaqueElement;
use selectors::bloom::BloomFilter;

use cssparser::{
    AtRuleParser, DeclarationParser, Parser as CssParser, ParserInput,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser,
    ToCss, CowRcStr, SourceLocation,
};
use std::collections::HashMap;
use std::fmt::{self, Write};
use std::borrow::Borrow;
use std::rc::Rc;

use precomputed_hash::PrecomputedHash;

pub fn phtml(html: &str) -> RcDom {
    parse_document(RcDom::default(), Default::default())
        .one(html)
}

pub fn walk<F>(node: &Handle, visitor: &mut F)
where
    F: FnMut(&Handle),
{
    visitor(node);

    for child in node.children.borrow().iter() {
        walk(child, visitor);
    }
}

pub fn element_text_contents(node: &Handle) -> String {
    let mut result = String::new();
    for child in node.children.borrow().iter() {
        if let NodeData::Text { contents } = &child.data {
            result.push_str(&contents.borrow());
        }
    }
    result
}

// css shit
#[derive(Debug)]
pub struct CssRule {
    pub selectors: Vec<Selector<TspImpl>>,
    pub declarations: HashMap<String, String>,
}

// --- DOM element wrapper for selector matching ---

#[derive(Clone, Debug)]
pub struct DomElement {
    pub node: Handle,
}

impl DomElement {
    pub fn get_attr(&self, attr_name: &str) -> Option<String> {
        if let NodeData::Element { attrs, .. } = &self.node.data {
            for attr in attrs.borrow().iter() {
                if attr.name.local.as_ref() == attr_name {
                    return Some(attr.value.to_string());
                }
            }
        }
        None
    }
}

impl SelectorElement for DomElement {
    type Impl = TspImpl;

    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::new(&*self.node)
    }

    fn parent_element(&self) -> Option<Self> {
        let old_parent = self.node.parent.take();
        let result = old_parent.as_ref().and_then(|w| {
            w.upgrade().and_then(|parent| {
                if let NodeData::Element { .. } = parent.data {
                    Some(DomElement { node: parent })
                } else {
                    None
                }
            })
        });
        self.node.parent.set(old_parent);
        result
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }

    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }

    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        let old_parent = self.node.parent.take();
        let result = old_parent.as_ref().and_then(|w| {
            let parent = w.upgrade()?;
            let children = parent.children.borrow();
            let pos = children.iter().position(|c| Rc::ptr_eq(c, &self.node))?;
            for child in children[..pos].iter().rev() {
                if let NodeData::Element { .. } = child.data {
                    return Some(DomElement { node: child.clone() });
                }
            }
            None
        });
        self.node.parent.set(old_parent);
        result
    }

    fn next_sibling_element(&self) -> Option<Self> {
        let old_parent = self.node.parent.take();
        let result = old_parent.as_ref().and_then(|w| {
            let parent = w.upgrade()?;
            let children = parent.children.borrow();
            let pos = children.iter().position(|c| Rc::ptr_eq(c, &self.node))?;
            for child in children[pos + 1..].iter() {
                if let NodeData::Element { .. } = child.data {
                    return Some(DomElement { node: child.clone() });
                }
            }
            None
        });
        self.node.parent.set(old_parent);
        result
    }

    fn first_element_child(&self) -> Option<Self> {
        for child in self.node.children.borrow().iter() {
            if let NodeData::Element { .. } = child.data {
                return Some(DomElement { node: child.clone() });
            }
        }
        None
    }

    fn is_html_element_in_html_document(&self) -> bool {
        true
    }

    fn has_local_name(&self, local_name: &<Self::Impl as SelectorImpl>::BorrowedLocalName) -> bool {
        if let NodeData::Element { name, .. } = &self.node.data {
            name.local.as_ref() == local_name
        } else {
            false
        }
    }

    fn has_namespace(&self, ns: &<Self::Impl as SelectorImpl>::BorrowedNamespaceUrl) -> bool {
        if let NodeData::Element { name, .. } = &self.node.data {
            name.ns.as_ref() == ns
        } else {
            false
        }
    }

    fn is_same_type(&self, other: &Self) -> bool {
        if let NodeData::Element { name: n1, .. } = &self.node.data {
            if let NodeData::Element { name: n2, .. } = &other.node.data {
                n1.ns == n2.ns && n1.local == n2.local
            } else {
                false
            }
        } else {
            false
        }
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&<Self::Impl as SelectorImpl>::NamespaceUrl>,
        local_name: &<Self::Impl as SelectorImpl>::LocalName,
        operation: &AttrSelectorOperation<&<Self::Impl as SelectorImpl>::AttrValue>,
    ) -> bool {
        match ns {
            NamespaceConstraint::Specific(url) if !url.0.is_empty() => return false,
            _ => {}
        }
        if let NodeData::Element { attrs, .. } = &self.node.data {
            for attr in attrs.borrow().iter() {
                if attr.name.local.as_ref() == local_name.0 {
                    return operation.eval_str(&attr.value);
                }
            }
        }
        false
    }

    fn match_non_ts_pseudo_class(
        &self,
        _pc: &<Self::Impl as SelectorImpl>::NonTSPseudoClass,
        _context: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        false
    }

    fn match_pseudo_element(
        &self,
        _pe: &<Self::Impl as SelectorImpl>::PseudoElement,
        _context: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        false
    }

    fn apply_selector_flags(&self, _flags: selectors::matching::ElementSelectorFlags) {}

    fn is_link(&self) -> bool {
        if let NodeData::Element { name, attrs, .. } = &self.node.data {
            if name.local.as_ref() == "a" {
                for attr in attrs.borrow().iter() {
                    if attr.name.local.as_ref() == "href" {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(
        &self,
        id: &<Self::Impl as SelectorImpl>::Identifier,
        _case_sensitivity: CaseSensitivity,
    ) -> bool {
        if let NodeData::Element { attrs, .. } = &self.node.data {
            for attr in attrs.borrow().iter() {
                if attr.name.local.as_ref() == "id" && attr.value.as_ref() == id.0 {
                    return true;
                }
            }
        }
        false
    }

    fn has_class(
        &self,
        name: &<Self::Impl as SelectorImpl>::Identifier,
        _case_sensitivity: CaseSensitivity,
    ) -> bool {
        if let NodeData::Element { attrs, .. } = &self.node.data {
            for attr in attrs.borrow().iter() {
                if attr.name.local.as_ref() == "class" {
                    for c in attr.value.split_whitespace() {
                        if c == name.0 {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn has_custom_state(&self, _name: &<Self::Impl as SelectorImpl>::Identifier) -> bool {
        false
    }

    fn imported_part(
        &self,
        _name: &<Self::Impl as SelectorImpl>::Identifier,
    ) -> Option<<Self::Impl as SelectorImpl>::Identifier> {
        None
    }

    fn is_part(&self, _name: &<Self::Impl as SelectorImpl>::Identifier) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        self.node.children.borrow().is_empty()
    }

    fn is_root(&self) -> bool {
        let old_parent = self.node.parent.take();
        let is_root = old_parent.is_none();
        self.node.parent.set(old_parent);
        is_root
    }

    fn add_element_unique_hashes(&self, _filter: &mut BloomFilter) -> bool {
        false
    }
}

pub fn rule_matches_element(rule: &CssRule, element: &DomElement) -> bool {
    let mut selector_caches = SelectorCaches::default();
    for selector in &rule.selectors {
        if matches_selector(
            selector,
            0,
            None,
            element,
            &mut MatchingContext::new(
                MatchingMode::Normal,
                None,
                &mut selector_caches,
                QuirksMode::NoQuirks,
                NeedsSelectorFlags::No,
                MatchingForInvalidation::No,
            ),
        ) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------
// SelectorImpl types

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct Atom(pub String);

impl Atom {
    #[allow(dead_code)]
    fn new(s: &str) -> Self {
        Atom(s.to_string())
    }
}

impl AsRef<str> for Atom {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Atom {
    fn from(s: &str) -> Self {
        Atom(s.to_string())
    }
}

impl ToCss for Atom {
    fn to_css<W: Write>(&self, dest: &mut W) -> fmt::Result {
        dest.write_str(&self.0)
    }
}

impl PrecomputedHash for Atom {
    fn precomputed_hash(&self) -> u32 {
        let mut h = 0u32;
        for b in self.0.bytes() {
            h = h.wrapping_mul(31).wrapping_add(b as u32);
        }
        h
    }
}

impl Borrow<str> for Atom {
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TspNonTSPseudoClass {}

impl NonTSPseudoClass for TspNonTSPseudoClass {
    type Impl = TspImpl;

    fn is_active_or_hover(&self) -> bool {
        false
    }

    fn is_user_action_state(&self) -> bool {
        false
    }
}

impl ToCss for TspNonTSPseudoClass {
    fn to_css<W: Write>(&self, _dest: &mut W) -> fmt::Result {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TspPseudoElement {}

impl PseudoElement for TspPseudoElement {
    type Impl = TspImpl;
}

impl ToCss for TspPseudoElement {
    fn to_css<W: Write>(&self, _dest: &mut W) -> fmt::Result {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TspImpl;

impl SelectorImpl for TspImpl {
    type ExtraMatchingData<'a> = ();
    type AttrValue = Atom;
    type Identifier = Atom;
    type LocalName = Atom;
    type NamespaceUrl = Atom;
    type NamespacePrefix = Atom;
    type BorrowedNamespaceUrl = str;
    type BorrowedLocalName = str;
    type NonTSPseudoClass = TspNonTSPseudoClass;
    type PseudoElement = TspPseudoElement;
}

// --- Selector parser for cssparser integration ---

struct TspSelectorParser;

impl<'i> selectors::parser::Parser<'i> for TspSelectorParser {
    type Impl = TspImpl;
    type Error = SelectorParseErrorKind<'i>;

    fn parse_non_ts_pseudo_class(
        &self,
        location: SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<TspNonTSPseudoClass, cssparser::ParseError<'i, Self::Error>> {
        Err(location.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClassOrElement(name)))
    }

    fn parse_non_ts_functional_pseudo_class<'t>(
        &self,
        name: CowRcStr<'i>,
        parser: &mut CssParser<'i, 't>,
        _after_part: bool,
    ) -> Result<TspNonTSPseudoClass, cssparser::ParseError<'i, Self::Error>> {
        Err(parser.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClassOrElement(name)))
    }

    fn parse_pseudo_element(
        &self,
        location: SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<TspPseudoElement, cssparser::ParseError<'i, Self::Error>> {
        Err(location.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClassOrElement(name)))
    }

    fn parse_functional_pseudo_element<'t>(
        &self,
        name: CowRcStr<'i>,
        parser: &mut CssParser<'i, 't>,
    ) -> Result<TspPseudoElement, cssparser::ParseError<'i, Self::Error>> {
        Err(parser.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClassOrElement(name)))
    }
}

// ---------------------------------------------------------
// CSS parsing

struct MyQualifiedRuleParser;

impl<'i> QualifiedRuleParser<'i> for MyQualifiedRuleParser {
    type Prelude = Vec<Selector<TspImpl>>;
    type QualifiedRule = CssRule;
    type Error = cssparser::ParseError<'i, ()>;

    fn parse_prelude<'t>(
        &mut self,
        input: &mut CssParser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        let start = input.position();
        while input.next().is_ok() {}
        let prelude = input.slice_from(start).to_string();

        let mut selector_input = ParserInput::new(&prelude);
        let mut selector_parser = CssParser::new(&mut selector_input);
        let selector_parser_impl = TspSelectorParser;

        // Просто пропускаем битые селекторы
        let list = match SelectorList::parse(
            &selector_parser_impl,
            &mut selector_parser,
            ParseRelative::No,
        ) {
            Ok(l) => l,
            Err(_) => return Ok(Vec::new()),
        };

        Ok(list.slice().to_vec())
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &cssparser::ParserState,
        input: &mut CssParser<'i, 't>,
    ) -> Result<Self::QualifiedRule, cssparser::ParseError<'i, Self::Error>> {
        let mut declarations = HashMap::new();
        let mut decl_impl = MyDeclarationParser;
        let mut decl_parser = RuleBodyParser::new(input, &mut decl_impl);
        while let Some(decl) = decl_parser.next() {
            match decl {
                Ok((name, value)) => {
                    declarations.insert(name, value);
                }
                Err(_) => continue,
            }
        }
        Ok(CssRule {
            selectors: prelude,
            declarations,
        })
    }
}

struct MyDeclarationParser;

impl<'i> DeclarationParser<'i> for MyDeclarationParser {
    type Declaration = (String, String);
    type Error = cssparser::ParseError<'i, ()>;

    fn parse_value<'t>(
        &mut self,
        name: cssparser::CowRcStr<'i>,
        input: &mut CssParser<'i, 't>,
        _declaration_start: &cssparser::ParserState,
    ) -> Result<Self::Declaration, cssparser::ParseError<'i, Self::Error>> {
        let start = input.position();
        while input.next().is_ok() {}
        let value = input.slice_from(start).to_string();
        Ok((name.to_string(), value.trim().to_string()))
    }
}

impl<'i> AtRuleParser<'i> for MyQualifiedRuleParser {
    type Prelude = ();
    type AtRule = CssRule;
    type Error = cssparser::ParseError<'i, ()>;
}

impl<'i> AtRuleParser<'i> for MyDeclarationParser {
    type Prelude = ();
    type AtRule = (String, String);
    type Error = cssparser::ParseError<'i, ()>;
}

impl<'i> QualifiedRuleParser<'i> for MyDeclarationParser {
    type Prelude = ();
    type QualifiedRule = (String, String);
    type Error = cssparser::ParseError<'i, ()>;
}

impl<'i> RuleBodyItemParser<'i, (String, String), cssparser::ParseError<'i, ()>> for MyDeclarationParser {
    fn parse_declarations(&self) -> bool { true }
    fn parse_qualified(&self) -> bool { false }
}

pub fn collect_css(dom: &RcDom) -> Vec<CssRule> {
    let mut all_rules = Vec::new();
    let mut collector = |node: &Handle| {
        if let NodeData::Element { name, .. } = &node.data {
            if name.local.as_ref() == "style" {
                let css_text = element_text_contents(node);
                let rules = parse_css(&css_text);
                all_rules.extend(rules);
            }
        }
    };
    walk(&dom.document, &mut collector);
    all_rules
}

pub fn parse_css(css_text: &str) -> Vec<CssRule> {
    let mut input = ParserInput::new(css_text);
    let mut parser = CssParser::new(&mut input);
    let mut rules = Vec::new();

    let mut rule_impl = MyQualifiedRuleParser;
    let mut rule_parser = StyleSheetParser::new(&mut parser, &mut rule_impl);
    while let Some(rule) = rule_parser.next() {
        if let Ok(rule) = rule {
            rules.push(rule);
        }
    }
    rules
}
