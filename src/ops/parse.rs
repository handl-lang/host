//! Rust `Function` impls for the `parse` module — the handls (handlers)
//! for the `op`/`handl` declarations in `parse.hnl`.
//!
//! The function is an FFI atom: it delegates to a registry of `DynParser`
//! impls (from `plugin-lib-parse`), specifically `Whatwg` and `Rfc` URL
//! parsers, and cannot be expressed as pure handl arrows without losing the
//! exact error messages the tests rely on. The `op`/`handl` declaration form
//! (ADR-0077 Phase 2) declares the interface in `parse.hnl`; this impl
//! provides the logic and is registered in an
//! [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).

use std::collections::HashMap;
use std::sync::Arc;

use handl_plugin_lib_handl::OpRegistry;

use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_parse::DynParser;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parser_name(value: &serde_json::Value) -> Result<String, EvalError> {
    let Some(obj) = value.as_object() else {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "parser object", "non-object",
        )));
    };

    let Some(ty) = obj.get("type").and_then(|v| v.as_str()) else {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "parser object with type", "object without type",
        )));
    };

    if ty != "parser" {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "type: parser", "non-parser",
        )));
    }

    let Some(name) = obj.get("name").and_then(|v| v.as_str()) else {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "parser object with name", "parser object without name",
        )));
    };

    Ok(name.to_string())
}

fn value_as_string(value: &serde_json::Value) -> Result<String, EvalError> {
    value.as_str().map(String::from).ok_or_else(|| {
        EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",))
    })
}

// ---------------------------------------------------------------------------
// Parse
// ---------------------------------------------------------------------------

/// The `parse.parse` handl — parse a string into a value using a named
/// parser (e.g. `"url_whatwg"`, `"url_rfc"`).
pub struct Parse {
    pub(crate) parsers: HashMap<String, Arc<dyn DynParser>>,
}

impl Function for Parse {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }

        let input = value_as_string(&args[0])?;
        let name = parser_name(&args[1])?;
        let parser = self
            .parsers
            .get(&name)
            .ok_or_else(|| EvalError::Other(format!("unknown parser: {name}")))?;

        let value = parser
            .parse(&input)
            .map_err(|e| EvalError::Other(e.to_string()))?;

        Ok(serde_json::Value::from(value))
    }
}

pub const PARSE_OPS: &[&str] = &["parse.parse"];

pub fn parse_op_registry() -> OpRegistry {
    let mut parsers: HashMap<String, Arc<dyn DynParser>> = HashMap::new();
    parsers.insert(
        "url_whatwg".to_string(),
        Arc::new(crate::ops::url_whatwg::Whatwg),
    );
    parsers.insert("url_rfc".to_string(), Arc::new(crate::ops::url_rfc::Rfc));
    let mut registry = OpRegistry::new();
    registry.register("parse.parse", Arc::new(Parse { parsers }));
    registry
}
