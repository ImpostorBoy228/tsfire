use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, RcDom};

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

#[derive(Debug)]
pub struct CssRule {
    pub selectors: Vec<String>,
    pub declarations: HashMap<String, String>,
}

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
