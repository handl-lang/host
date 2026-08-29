use crate::{EofFactory, TokenKind};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum NumSrcKind {
    Number(f64),
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StrSrcKind {
    Str(String),
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnitSrcKind {
    Bang,
    Eof,
}

crate::compose_token_kind! {
    /// A small composed kind for testing the macro.
    pub enum ComposedKind {
        factory: ComposedFactory;

        Eof,
        Number(f64),
        String(String),
        Bang,
    }

    NumSrcKind {
        Number(v) => Number(v),
        Eof => Eof,
    }
    StrSrcKind {
        Str(v) => String(v),
        Eof => Eof,
    }
    UnitSrcKind {
        Bang => Bang,
        Eof => Eof,
    }
}

impl fmt::Display for ComposedKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComposedKind::Number(n) => write!(f, "number({n})"),
            ComposedKind::String(s) => write!(f, "string({s})"),
            ComposedKind::Bang => write!(f, "bang"),
            ComposedKind::Eof => write!(f, "eof"),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ComposedFactory;

impl EofFactory<ComposedKind> for ComposedFactory {
    fn eof_kind(&self) -> ComposedKind {
        ComposedKind::Eof
    }
}

#[test]
fn from_num_src_kind() {
    assert_eq!(
        ComposedKind::from(NumSrcKind::Number(42.0)),
        ComposedKind::Number(42.0)
    );
    assert_eq!(ComposedKind::from(NumSrcKind::Eof), ComposedKind::Eof);
}

#[test]
fn from_str_src_kind() {
    assert_eq!(
        ComposedKind::from(StrSrcKind::Str("hi".to_string())),
        ComposedKind::String("hi".to_string())
    );
    assert_eq!(ComposedKind::from(StrSrcKind::Eof), ComposedKind::Eof);
}

#[test]
fn from_unit_src_kind() {
    assert_eq!(ComposedKind::from(UnitSrcKind::Bang), ComposedKind::Bang);
    assert_eq!(ComposedKind::from(UnitSrcKind::Eof), ComposedKind::Eof);
}

#[test]
fn token_kind_is_eof() {
    assert!(ComposedKind::Eof.is_eof());
    assert!(!ComposedKind::Number(1.0).is_eof());
    assert!(!ComposedKind::String("x".to_string()).is_eof());
    assert!(!ComposedKind::Bang.is_eof());
}

#[test]
fn token_kind_is_error() {
    assert!(!ComposedKind::Eof.is_error());
    assert!(!ComposedKind::Number(1.0).is_error());
    assert!(!ComposedKind::Bang.is_error());
}

#[test]
fn token_kind_display() {
    assert_eq!(ComposedKind::Eof.to_string(), "eof");
    assert_eq!(ComposedKind::Number(1.0).to_string(), "number(1)");
    assert_eq!(ComposedKind::Bang.to_string(), "bang");
}

#[test]
fn token_kind_factory_type() {
    let _: <ComposedKind as TokenKind>::Factory = ComposedFactory;
}
