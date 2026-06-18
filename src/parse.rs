use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, RcDom, NodeData};

use selectors::Element as SelectorElement;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::matching::{MatchingContext, MatchingMode, matches_selector};
use std::fmt;

use cssparser::{
    AtRuleParser, DeclarationParser, Parser as CssParser, ParserInput,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser,
};
use std::collections::HashMap;

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

// css shit
#[derive(Debug)]
pub struct CssRule {
    pub selectors: Vec<String>,
    pub declarations: HashMap<String, String>,
}

pub struct DomElement<'a> {
    pub node: &'a Handle,
}

impl<'a> SelectorElement for DomElement<'a> {
    type Impl = ();

    fn parent_element(&self) -> Option<Self> {
        // Ищем родителя, который является элементом
        let parent = self.node.borrow().parent.clone();
        parent.and_then(|p| {
            let p_borrowed = p.borrow();
            if let NodeData::Element { .. } = p_borrowed.data {
                Some(DomElement { node: &p })
            } else {
                None
            }
        })
    }

    fn has_local_name(&self, local_name: &selectors::parser::LocalName) -> bool {
        if let NodeData::Element { name, .. } = &self.node.borrow().data {
            name.local.as_ref() == local_name.as_str()
        } else {
            false
        }
    }

    fn get_id(&self) -> Option<CaseSensitivity> {
        // Ищем атрибут id
        self.get_attr("id")
            .map(|_| CaseSensitivity::CaseSensitive)
    }

    fn has_class(&self, name: &str, case_sensitivity: CaseSensitivity) -> bool {
        if let Some(class_attr) = self.get_attr("class") {
            // Простой split по пробелам (в реальности нужно учитывать несколько пробелов и табуляцию)
            class_attr.split_whitespace().any(|c| c == name)
        } else {
            false
        }
    }

    fn is_html_element(&self) -> bool {
        true // для простоты считаем все элементы HTML
    }

    fn is_shadow_host(&self) -> bool {
        false
    }

    fn is_visited_link(&self) -> bool {
        false
    }

    fn is_active_link(&self) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        // Проверяем, нет ли дочерних элементов или текстовых узлов
        // Здесь нужно реализовать корректно для псевдокласса :empty
        // Упрощённо: считаем пустым, если нет дочерних узлов вообще
        self.node.children.borrow().is_empty()
    }

    // Вспомогательный метод для получения значения атрибута
    fn get_attr(&self, attr_name: &str) -> Option<String> {
        if let NodeData::Element { attrs, .. } = &self.node.borrow().data {
            for attr in attrs.borrow().iter() {
                if attr.name.local.as_ref() == attr_name {
                    return Some(attr.value.to_string());
                }
            }
        }
        None
    }

    // Другие методы трейта можно оставить с реализациями по умолчанию,
    // но мы их переопределим для более точного сопоставления:
    fn attr_matches(
        &self,
        _ns: &NamespaceConstraint<&Namespace>,
        local_name: &selectors::parser::LocalName,
        operation: &AttrSelectorOperation<&str>,
    ) -> bool {
        // Проверяем атрибутные селекторы (например, [class="foo"])
        if let Some(value) = self.get_attr(local_name.as_str()) {
            match operation {
                AttrSelectorOperation::Exists => true,
                AttrSelectorOperation::Equals(val) => value == *val,
                AttrSelectorOperation::Includes(val) => {
                    value.split_whitespace().any(|part| part == *val)
                }
                AttrSelectorOperation::DashMatch(val) => {
                    value == *val || value.starts_with(&format!("{}-", val))
                }
                AttrSelectorOperation::Prefix(val) => value.starts_with(val),
                AttrSelectorOperation::Suffix(val) => value.ends_with(val),
                AttrSelectorOperation::Substring(val) => value.contains(val),
            }
        } else {
            false
        }
    }

    // Также нужно реализовать методы для псевдоклассов, но для начала оставим заглушки.
}

pub fn rule_matches_element(rule: &CssRule, element: &DomElement) -> bool {
    let context = MatchingContext::new(MatchingMode::Normal);
    for selector in &rule.selectors {
        if matches_selector(selector, element, &context, &mut |_, _| false) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------

struct MyQualifiedRuleParser;

impl<'i> QualifiedRuleParser<'i> for MyQualifiedRuleParser {
    type Prelude = Vec<String>;
    type QualifiedRule = CssRule;
    type Error = cssparser::ParseError<'i, ()>;

    fn parse_prelude<'t>(
        &mut self,
        input: &mut CssParser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        let start = input.position();
        while input.next().is_ok() {}
        let prelude = input.slice_from(start).to_string();
        let selectors: Vec<String> = prelude
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(selectors)
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
        let value = input
            .try_parse(|input| {
                let start = input.position();
                while let Ok(_) = input.expect_ident() {
                }
                let slice = input.slice_from(start);
                Ok(slice.to_string())
            })
            .unwrap_or_else(|_: cssparser::ParseError<'i, ()>| "".to_string());
        Ok((name.to_string(), value))
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
