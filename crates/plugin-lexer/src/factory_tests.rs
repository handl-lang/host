use crate::{
    DefaultLexError, EofFactory, IdentFactory, KindFactory, NumberFactory, Span, TokenKind,
};
use std::fmt;
use std::iter::Peekable;

#[derive(Debug, Clone, PartialEq)]
struct DummyKind(String);

impl fmt::Display for DummyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TokenKind for DummyKind {
    type Factory = DummyFactory;
    fn is_eof(&self) -> bool {
        self.0 == "eof"
    }
    fn is_error(&self) -> bool {
        false
    }
}

#[derive(Clone)]
struct DummyFactory;

impl EofFactory<DummyKind> for DummyFactory {
    fn eof_kind(&self) -> DummyKind {
        DummyKind("eof".to_string())
    }
}

impl KindFactory<DummyKind> for DummyFactory {
    fn string_kind(&self, value: String) -> DummyKind {
        DummyKind(format!("string({value})"))
    }
    fn number_kind(&self, value: f64) -> DummyKind {
        DummyKind(format!("number({value})"))
    }
    fn ident_kind(&self, text: String) -> DummyKind {
        DummyKind(format!("ident({text})"))
    }
    fn keyword_kind(&self, _text: &str) -> Option<DummyKind> {
        None
    }
    fn operator_kind(&self, _text: &str) -> Option<DummyKind> {
        None
    }
}

impl NumberFactory<DummyKind> for DummyFactory {
    fn is_number_start(&self, ch: char) -> bool {
        ch.is_ascii_digit()
    }
    fn lex_number(
        &self,
        _chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(f64, Span), DefaultLexError> {
        Ok((0.0, Span::default()))
    }
}

impl IdentFactory<DummyKind> for DummyFactory {
    fn is_ident_start(&self, ch: char) -> bool {
        ch.is_alphabetic()
    }
    fn is_ident_continue(&self, ch: char) -> bool {
        ch.is_alphanumeric()
    }
    fn lex_ident(
        &self,
        _chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(String, Span), DefaultLexError> {
        Ok(("".to_string(), Span::default()))
    }
}

#[test]
fn number_factory_default_regex() {
    let factory = DummyFactory;
    assert_eq!(factory.number_regex(), r"-?\d+(\.\d+)?([eE][+-]?\d+)?");
}

#[test]
fn ident_factory_default_regex() {
    let factory = DummyFactory;
    assert_eq!(factory.identifier_regex(), r"[a-zA-Z_][a-zA-Z0-9_]*");
}

#[test]
fn number_factory_regex_can_be_overridden() {
    #[derive(Clone)]
    struct CustomFactory;
    impl EofFactory<DummyKind> for CustomFactory {
        fn eof_kind(&self) -> DummyKind { DummyKind("eof".to_string()) }
    }
    impl KindFactory<DummyKind> for CustomFactory {
        fn string_kind(&self, v: String) -> DummyKind { DummyKind(v) }
        fn number_kind(&self, v: f64) -> DummyKind { DummyKind(v.to_string()) }
        fn ident_kind(&self, v: String) -> DummyKind { DummyKind(v) }
        fn keyword_kind(&self, _: &str) -> Option<DummyKind> { None }
        fn operator_kind(&self, _: &str) -> Option<DummyKind> { None }
    }
    impl NumberFactory<DummyKind> for CustomFactory {
        fn is_number_start(&self, ch: char) -> bool { ch.is_ascii_digit() }
        fn lex_number(&self, _: &mut Peekable<std::str::CharIndices<'_>>) -> Result<(f64, Span), DefaultLexError> { Ok((0.0, Span::default())) }
        fn number_regex(&self) -> &str { r"0x[0-9a-fA-F]+" }
    }

    let factory = CustomFactory;
    assert_eq!(factory.number_regex(), r"0x[0-9a-fA-F]+");
}

#[test]
fn ident_factory_regex_can_be_overridden() {
    #[derive(Clone)]
    struct CustomFactory;
    impl EofFactory<DummyKind> for CustomFactory {
        fn eof_kind(&self) -> DummyKind { DummyKind("eof".to_string()) }
    }
    impl KindFactory<DummyKind> for CustomFactory {
        fn string_kind(&self, v: String) -> DummyKind { DummyKind(v) }
        fn number_kind(&self, v: f64) -> DummyKind { DummyKind(v.to_string()) }
        fn ident_kind(&self, v: String) -> DummyKind { DummyKind(v) }
        fn keyword_kind(&self, _: &str) -> Option<DummyKind> { None }
        fn operator_kind(&self, _: &str) -> Option<DummyKind> { None }
    }
    impl IdentFactory<DummyKind> for CustomFactory {
        fn is_ident_start(&self, ch: char) -> bool { ch.is_alphabetic() }
        fn is_ident_continue(&self, ch: char) -> bool { ch.is_alphanumeric() }
        fn lex_ident(&self, _: &mut Peekable<std::str::CharIndices<'_>>) -> Result<(String, Span), DefaultLexError> { Ok(("".into(), Span::default())) }
        fn identifier_regex(&self) -> &str { r"[$][a-zA-Z0-9_]*" }
    }

    let factory = CustomFactory;
    assert_eq!(factory.identifier_regex(), r"[$][a-zA-Z0-9_]*");
}
