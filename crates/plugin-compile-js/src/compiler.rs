//! The `JsCompiler` implementation.

use handl_plugin_compile::{CompileError, Compiler, CompilerConfig, CompilerInput, CompilerOutput};

use crate::config::JsConfig;
use crate::emit::JsEmitter;
use crate::emit::source_map::{SourceMapBuilder, span_to_line_col};
use crate::lower;
use crate::output::JsOutput;
use crate::runtime::prelude::runtime_prelude;

/// JavaScript compiler for the Handl arrow calculus.
#[derive(Debug, Default, Clone, Copy)]
pub struct JsCompiler;

impl Compiler for JsCompiler {
    fn compile(
        &self,
        input: &CompilerInput,
        config: &dyn CompilerConfig,
    ) -> Result<Box<dyn CompilerOutput>, CompileError> {
        let js_config = config
            .as_any()
            .downcast_ref::<JsConfig>()
            .ok_or_else(|| CompileError::InvalidInput("expected JsConfig".to_string()))?;

        let main = lower::with_config(js_config, || lower::lower(&input.term))?;
        let emitter = JsEmitter;
        let main_body = emitter.emit_secure(&main)?;
        let wrapped = js_config.format.wrap(&main_body);
        let mut full = if js_config.format.name() == "esm" || js_config.format.name() == "commonjs"
        {
            let prelude = runtime_prelude(js_config.format.as_ref());
            format!("{}{}", prelude, wrapped)
        } else {
            wrapped
        };

        if js_config.source_map {
            let map_name = "main.js.map";
            full.push_str(&format!("\n//# sourceMappingURL={map_name}\n"));

            let (source_line, source_col) = input
                .ast
                .get(input.root)
                .and_then(|n| n.span)
                .map(|s| span_to_line_col(&input.source, s))
                .unwrap_or((0, 0));

            let mut builder = SourceMapBuilder::new("<source>", &input.source, "main.js");
            builder.add_mapping(0, 0, source_line, source_col, None);
            let map = builder.build();

            return Ok(Box::new(JsOutput::new_with_source_map(full, map, map_name)));
        }

        Ok(Box::new(JsOutput::new(full)))
    }

    fn target(&self) -> &str {
        "js"
    }
}

/// Convenience: compile `input` with `config` and return the generated JS string.
pub fn compile_to_string(input: &CompilerInput, config: &JsConfig) -> Result<String, CompileError> {
    let compiler = JsCompiler;
    let output = compiler.compile(input, config)?;
    match output.primary_output() {
        Some(bytes) => Ok(String::from_utf8_lossy(bytes).to_string()),
        None => Err(CompileError::Emit("no primary JS output".to_string())),
    }
}
