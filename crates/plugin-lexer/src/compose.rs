//! Declarative macro for composing a unified token-kind enum from
//! individual token-plugin kinds, eliminating hand-written enum
//! definitions, `From` impls, and `TokenKind` impls.

/// Generates a unified token kind enum, `From` impls for each source
/// kind, and a `TokenKind` impl from a declarative spec.
///
/// The macro emits the unified enum with `#[derive(Debug, Clone, PartialEq)]`,
/// a `TokenKind` impl, and a `From<SourceKind>` impl for each source kind
/// mapping block. Each `From` impl uses an if-let chain that qualifies
/// patterns with the source type and targets with the unified type.
///
/// # Example
///
/// ```
/// use handl_plugin_lexer::{EofFactory, TokenKind, compose_token_kind};
/// use std::fmt;
///
/// #[derive(Debug, Clone, PartialEq)]
/// pub enum NumSrc { Number(f64), Eof }
///
/// #[derive(Debug, Clone, PartialEq)]
/// pub enum StrSrc { Str(String), Eof }
///
/// compose_token_kind! {
///     pub enum MyKind {
///         factory: MyFactory;
///
///         Eof,
///         Number(f64),
///         String(String),
///     }
///
///     NumSrc {
///         Number(v) => Number(v),
///         Eof => Eof,
///     }
///     StrSrc {
///         Str(v) => String(v),
///         Eof => Eof,
///     }
/// }
///
/// impl fmt::Display for MyKind {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         match self {
///             MyKind::Number(n) => write!(f, "number({n})"),
///             MyKind::String(s) => write!(f, "string({s})"),
///             MyKind::Eof => write!(f, "eof"),
///         }
///     }
/// }
///
/// #[derive(Debug, Default, Clone, Copy)]
/// pub struct MyFactory;
///
/// impl EofFactory<MyKind> for MyFactory {
///     fn eof_kind(&self) -> MyKind { MyKind::Eof }
/// }
/// ```
#[macro_export]
macro_rules! compose_token_kind {
    (
        $(#[$doc:meta])*
        $vis:vis enum $name:ident {
            factory: $factory:ty;
            $($variants:tt)*
        }
        $($rest:tt)*
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq)]
        $vis enum $name {
            $($variants)*
        }

        impl $crate::TokenKind for $name {
            type Factory = $factory;

            fn is_eof(&self) -> bool {
                matches!(self, $name::Eof)
            }

            fn is_error(&self) -> bool {
                false
            }
        }

        $crate::compose_from_impls!($name; $($rest)*);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! compose_from_impls {
    ($name:ident;) => {};
    (
        $name:ident;
        $src:ident { $($arms:tt)* }
        $($rest:tt)*
    ) => {
        impl ::core::convert::From<$src> for $name {
            fn from(k: $src) -> Self {
                $crate::compose_arms!(k; $src; $name; $($arms)*);
                unreachable!("unhandled variant in From<{}>", stringify!($src))
            }
        }
        $crate::compose_from_impls!($name; $($rest)*);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! compose_arms {
    ($k:ident; $src:ident; $name:ident;) => {};
    ($k:ident; $src:ident; $name:ident; $pat:ident($bind:ident) => $tgt:ident($($args:tt)*), $($rest:tt)*) => {
        if let $src::$pat($bind) = $k { return $name::$tgt($($args)*); }
        $crate::compose_arms!($k; $src; $name; $($rest)*);
    };
    ($k:ident; $src:ident; $name:ident; $pat:ident($bind:ident) => $tgt:ident, $($rest:tt)*) => {
        if let $src::$pat($bind) = $k { return $name::$tgt; }
        $crate::compose_arms!($k; $src; $name; $($rest)*);
    };
    ($k:ident; $src:ident; $name:ident; $pat:ident => $tgt:ident($($args:tt)*), $($rest:tt)*) => {
        if let $src::$pat = $k { return $name::$tgt($($args)*); }
        $crate::compose_arms!($k; $src; $name; $($rest)*);
    };
    ($k:ident; $src:ident; $name:ident; $pat:ident => $tgt:ident, $($rest:tt)*) => {
        if let $src::$pat = $k { return $name::$tgt; }
        $crate::compose_arms!($k; $src; $name; $($rest)*);
    };
}
