use std::collections::HashMap;
use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_parse::DynParser;
use handl_plugin_lib_url_rfc::Rfc;
use handl_plugin_lib_url_whatwg::Whatwg;

/// The `parse` runtime module.
///
/// Provides `parse.parse(input, parser)`.
pub struct ParseModule {
    parse: Parse,
}

impl ParseModule {
    /// Creates a `ParseModule` with the default parsers.
    pub fn new() -> Self {
        let mut parsers: HashMap<String, Arc<dyn DynParser>> = HashMap::new();
        parsers.insert("url_whatwg".to_string(), Arc::new(Whatwg));
        parsers.insert("url_rfc".to_string(), Arc::new(Rfc));
        Self {
            parse: Parse { parsers },
        }
    }
}

impl Default for ParseModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ParseModule {
    fn name(&self) -> &str {
        "parse"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "parse" => Some(&self.parse),
            _ => None,
        }
    }
}

struct Parse {
    parsers: HashMap<String, Arc<dyn DynParser>>,
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
