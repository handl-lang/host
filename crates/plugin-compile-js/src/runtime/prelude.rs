//! Runtime prelude for primitive Handl operations in JavaScript.

use crate::config::JsOutputFormat;

/// Return the runtime prelude source for the requested output format.
pub fn runtime_prelude(format: &dyn JsOutputFormat) -> String {
    let decl = if format.name() == "esm" || format.name() == "iife" {
        "const"
    } else {
        "var"
    };
    format!(
        r#"{} __handl_rt = {{
  mkPair: (a, b) => ({{__pair__: [a, b]}}),
  mkLeft: (v) => ({{__either__: "left", value: v}}),
  mkRight: (v) => ({{__either__: "right", value: v}}),
  isPair: (v) => (typeof v === "object" && v !== null && Array.isArray(v.__pair__)),
  fst: (p) => p.__pair__[0],
  snd: (p) => p.__pair__[1],
  isLeft: (v) => (typeof v === "object" && v !== null && v.__either__ === "left"),
  isRight: (v) => (typeof v === "object" && v !== null && v.__either__ === "right"),
  leftValue: (v) => v.value,
  rightValue: (v) => v.value,
  add: (a, b) => (a + b),
  sub: (a, b) => (a - b),
  mul: (a, b) => (a * b),
  div: (a, b) => (a / b),
  concat: (a, b) => (Array.isArray(a) ? a.concat(b) : String(a) + String(b)),
  projEnv: (env, name) => (env[name]),
  child: (input, role) => input[role],
  handle: (body, target, impl) => (input) => (typeof impl === "function" ? impl(input) : impl),
  rewrite: (source, target, witness) => source,
  path: (from, to, witness) => from,
  equiv: (a, b, path) => a,
  jsCall: (module, name, args, _descriptors) => {{
    const host = globalThis[module];
    if (!host || typeof host[name] !== "function") {{
      throw new Error("unsafe or missing foreign call");
    }}
    return host[name](...args);
  }}
}};
"#,
        decl
    )
}
