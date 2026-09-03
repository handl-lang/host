use handl_platform::Function;
use handl_platform::Module;

use crate::r#type::coercion::{ToBool, ToNumber, ToString};
use crate::r#type::predicates::{
    ANY_TAG, ARR_TAG, ARRAY_TAG, BOOL_TAG, NULL_TAG, NUM_TAG, NUMBER_TAG, OBJ_TAG, OBJECT_TAG,
    STR_TAG, STRING_TAG, TypeIs, TypeMay, TypeMust, TypeNot, TypeOf, TypeWhen, UNKNOWN_TAG,
};
use crate::r#type::utilities::{Coalesce, Default, Identity};

/// The `typeof` module — type coercion and utility functions.
///
/// Provides functions callable as `typeof.to_string(v)`, `typeof.to_number(v)`,
/// `typeof.to_bool(v)`, `typeof.type_of(v)`, `typeof.identity(v)`,
/// `typeof.default(v)`, `typeof.coalesce(v)` in handl expressions.
///
/// Note: the module was originally named `type`, but `type` became a keyword
/// in v0.1 (ADR-0074), so the module was renamed to `typeof` to avoid the
/// conflict. The function `typeof.type_of(v)` is the primary type-checking
/// function used in handl source files.
///
/// # NaN and Infinity
///
/// `serde_json::Value` cannot represent NaN or Infinity — they serialize as
/// `null`. Values that would be NaN or Infinity are therefore handled by the
/// `null` branch in each function, yielding `"null"`, `0.0`, and `false`
/// respectively. This is a deliberate graceful-degradation choice, not a bug.
#[derive(Clone, Copy)]
pub struct TypeModule;

impl Module for TypeModule {
    fn name(&self) -> &str {
        "typeof"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            // base type tags
            "string" => Some(&STRING_TAG),
            "str" => Some(&STR_TAG),
            "number" => Some(&NUMBER_TAG),
            "num" => Some(&NUM_TAG),
            "bool" => Some(&BOOL_TAG),
            "null" => Some(&NULL_TAG),
            "array" => Some(&ARRAY_TAG),
            "arr" => Some(&ARR_TAG),
            "object" => Some(&OBJECT_TAG),
            "obj" => Some(&OBJ_TAG),
            "any" => Some(&ANY_TAG),
            "unknown" => Some(&UNKNOWN_TAG),
            // mood-style type predicates
            "is" => Some(&TypeIs),
            "must" => Some(&TypeMust),
            "may" => Some(&TypeMay),
            "not" => Some(&TypeNot),
            "when" => Some(&TypeWhen),
            // coercion
            "to_string" => Some(&ToString),
            "to_number" => Some(&ToNumber),
            "to_bool" => Some(&ToBool),
            // predicates
            "type_of" => Some(&TypeOf),
            "typeof" => Some(&TypeOf),
            // utilities
            "identity" => Some(&Identity),
            "default" => Some(&Default),
            "coalesce" => Some(&Coalesce),
            _ => None,
        }
    }
}
