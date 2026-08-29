use std::collections::HashMap;
use std::sync::Arc;

use handl_plugin_lib_parse::DynParser;
use handl_plugin_lib_url_rfc::Rfc;
use handl_plugin_lib_url_whatwg::Whatwg;
use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::NonFunctionValue;

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
        Self { parse: Parse { parsers } }
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

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "parse" => Some(&self.parse),
            _ => None,
        }
    }
}

struct Parse {
    parsers: HashMap<String, Arc<dyn DynParser>>,
}

impl Function<NonFunctionValue> for Parse {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, handl_plugin_vm::Error<NonFunctionValue>> {
        if args.len() != 2 {
            return Err(handl_plugin_vm::Error::ArgumentCount {
                expected: 2,
                got: args.len(),
            });
        }

        let input = value_as_string(&args[0])?;
        let name = parser_name(&args[1])?;
        let parser = self
            .parsers
            .get(&name)
            .ok_or_else(|| handl_plugin_vm::Error::PrimitiveError(format!("unknown parser: {name}")))?;

        let value = parser
            .parse(&input)
            .map_err(|e| handl_plugin_vm::Error::PrimitiveError(e.to_string()))?;

        Ok(NonFunctionValue::new(value))
    }
}

fn parser_name(value: &NonFunctionValue) -> Result<String, handl_plugin_vm::Error<NonFunctionValue>> {
    let Some(obj) = value.as_object() else {
        return Err(handl_plugin_vm::Error::TypeMismatch {
            expected: "parser object",
            got: "non-object",
        });
    };

    let Some(ty) = obj.get("type").and_then(|v| v.as_str()) else {
        return Err(handl_plugin_vm::Error::TypeMismatch {
            expected: "parser object with type",
            got: "object without type",
        });
    };

    if ty != "parser" {
        return Err(handl_plugin_vm::Error::TypeMismatch {
            expected: "type: parser",
            got: "non-parser",
        });
    }

    let Some(name) = obj.get("name").and_then(|v| v.as_str()) else {
        return Err(handl_plugin_vm::Error::TypeMismatch {
            expected: "parser object with name",
            got: "parser object without name",
        });
    };

    Ok(name.to_string())
}

fn value_as_string(
    value: &NonFunctionValue,
) -> Result<String, handl_plugin_vm::Error<NonFunctionValue>> {
    value
        .as_str()
        .map(String::from)
        .ok_or_else(|| handl_plugin_vm::Error::TypeMismatch {
            expected: "string",
            got: "non-string",
        })
}
