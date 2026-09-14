//! JS compiler target and output-format configuration.

use std::any::Any;
use std::fmt;

use handl_plugin_compile::CompilerConfig;

/// A JavaScript runtime target.
pub trait JsTarget: fmt::Debug + Send + Sync {
    /// The canonical target name.
    fn name(&self) -> &str;
}

/// The Deno JavaScript runtime.
#[derive(Debug, Clone, Copy, Default)]
pub struct Deno;

impl JsTarget for Deno {
    fn name(&self) -> &str {
        "deno"
    }
}

/// The Node.js JavaScript runtime.
#[derive(Debug, Clone, Copy, Default)]
pub struct Node;

impl JsTarget for Node {
    fn name(&self) -> &str {
        "node"
    }
}

/// The Bun JavaScript runtime.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bun;

impl JsTarget for Bun {
    fn name(&self) -> &str {
        "bun"
    }
}

/// The Browser JavaScript runtime target.
#[derive(Debug, Clone, Copy, Default)]
pub struct Browser;

impl JsTarget for Browser {
    fn name(&self) -> &str {
        "browser"
    }
}

/// A JavaScript module output format.
pub trait JsOutputFormat: fmt::Debug + Send + Sync {
    /// The canonical format name.
    fn name(&self) -> &str;

    /// Wrap a generated expression body in the module boilerplate.
    fn wrap(&self, body: &str) -> String;
}

/// ECMAScript module output.
#[derive(Debug, Clone, Copy, Default)]
pub struct Esm;

impl JsOutputFormat for Esm {
    fn name(&self) -> &str {
        "esm"
    }

    fn wrap(&self, body: &str) -> String {
        format!("export default {};\n", body)
    }
}

/// CommonJS module output.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommonJs;

impl JsOutputFormat for CommonJs {
    fn name(&self) -> &str {
        "commonjs"
    }

    fn wrap(&self, body: &str) -> String {
        format!("module.exports = {};\n", body)
    }
}

/// IIFE output, bundling the runtime prelude and returning the expression.
#[derive(Debug, Clone, Copy, Default)]
pub struct Iife;

impl JsOutputFormat for Iife {
    fn name(&self) -> &str {
        "iife"
    }

    fn wrap(&self, body: &str) -> String {
        let prelude = crate::runtime::prelude::runtime_prelude(self);
        format!(
            "(function() {{\n{}{}  return {};\n}})();\n",
            prelude,
            if prelude.ends_with('\n') { "" } else { "\n" },
            body
        )
    }
}

/// Inline output, no module header; ideal for `<script>` or `eval`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inline;

impl JsOutputFormat for Inline {
    fn name(&self) -> &str {
        "inline"
    }

    fn wrap(&self, body: &str) -> String {
        let prelude = crate::runtime::prelude::runtime_prelude(self);
        format!("{}{}", prelude, body)
    }
}

/// Configuration for the JavaScript compiler.
#[derive(Debug)]
pub struct JsConfig {
    /// The target runtime (Deno, Node, Bun, Browser, ...).
    pub target: Box<dyn JsTarget>,
    /// The module output format (ESM, CommonJS, IIFE, Inline).
    pub format: Box<dyn JsOutputFormat>,
    /// Whether debug information should be emitted.
    pub debug: bool,
    /// Keep the runtime `js` client for all foreign calls.
    ///
    /// When `true`, no foreign calls are inlined and all calls go through
    /// `__handl_rt.jsCall`.
    pub no_inline_foreign: bool,
    /// Whether a Source Map V3 should be emitted.
    pub source_map: bool,
}

impl JsConfig {
    /// Create a config for Deno + ESM.
    pub fn esm_deno() -> Self {
        Self {
            target: Box::new(Deno),
            format: Box::new(Esm),
            debug: false,
            no_inline_foreign: false,
            source_map: false,
        }
    }

    /// Create a config for Node + CommonJS.
    pub fn cjs_node() -> Self {
        Self {
            target: Box::new(Node),
            format: Box::new(CommonJs),
            debug: false,
            no_inline_foreign: false,
            source_map: false,
        }
    }
}

impl CompilerConfig for JsConfig {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn target(&self) -> &str {
        "js"
    }

    fn debug(&self) -> bool {
        self.debug
    }

    fn source_map(&self) -> bool {
        self.source_map
    }
}
