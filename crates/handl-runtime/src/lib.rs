//! # handl-runtime
//!
//! Binds a [`Language`] to engine impls and exposes staged `lex`/`parse`/`eval`
//! over a `Language` + stage inputs (ADR-0044 §3, §7).
//!
//! A [`Runtime`] owns a [`Language`] and optional engine impls for each stage
//! (lex, parse, eval). Each stage dispatches to its bound engine or returns
//! [`LangError::NotImplemented`] if no engine is attached. This enables
//! **eval-only runtimes** (a JIT/cranelift runtime that compiles a pre-parsed
//! [`Ast`]) as well as full `lex → parse → eval` pipelines.
//!
//! ## Engine selection as data (§7)
//!
//! Engine selection is **data-driven**, not hardcoded host-code linking. A
//! [`RuntimeConfig`] carries engine names as strings (e.g. `"pratt"`,
//! `"tree-walk"`); [`Runtime::from_config`] resolves those names to concrete
//! engine crates at build time. Swapping an engine is a config change — the
//! `Language` and all fragments stay untouched.
//!
//! ### Phase 3 resolvable names
//!
//! | Stage | Name         | Engine crate                          |
//! |-------|--------------|---------------------------------------|
//! | parse | `"pratt"`    | `plugin-engine-parse-pratt`          |
//! | eval  | `"tree-walk"`| `plugin-engine-eval-tree-walk`       |
//! | eval  | `"bytecode"` | `plugin-engine-eval-bytecode`        |
//! | eval  | `"cranelift"`| `plugin-engine-eval-cranelift`       |
//! | lex   | `"data"`     | `plugin-engine-lex-pattern`             |
//!
//! Unknown names for any stage return [`LangError::Engine`].
//!
//! ## Dyn-compatibility note
//!
//! [`ParserEngine`] is not dyn-compatible (its `expression` method references
//! `Self` via [`ParserContext`]). The [`ParseEngineDyn`] wrapper trait
//! exposes only the dyn-compatible `parse` method, allowing parse engines to
//! be stored as trait objects. [`LexerEngine`] and [`EvalEngine`] are already
//! dyn-compatible and need no wrapper.

#![warn(missing_docs)]

pub mod meta;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use handl_language::{Ast, LangError, Language, Source, Tokens, Value};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest, HostFunctions};
use handl_plugin_engine_lex::{LexerEngine, LexerRequest};
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_engine_eval_bytecode::BytecodeEngine;
use handl_plugin_engine_eval_cranelift::CraneliftEngine;
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;

/// Data-driven engine selection for a [`Runtime`] (ADR-0044 §7).
///
/// Each field is an optional engine name. `None` leaves that stage's engine
/// unbound (the stage returns [`LangError::NotImplemented`] at runtime).
/// `Some(name)` requests a specific engine, resolved by
/// [`Runtime::from_config`] to a concrete engine crate at build time.
///
/// This is serializable so a kit manifest or runtime configuration file can
/// carry it as data — swapping an engine is a config change, not a recompile.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// The lex engine name (e.g. `"data"` for the data-driven lexer engine).
    pub lex: Option<String>,
    /// The parse engine name (e.g. `"pratt"` or `"chumsky"`).
    pub parse: Option<String>,
    /// The eval engine name (e.g. `"tree-walk"`, `"bytecode"`, `"cranelift"`).
    pub eval: Option<String>,
}

impl RuntimeConfig {
    /// Create an empty config with no engines selected (all stages unbound).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the lex engine name (builder style).
    pub fn with_lex(mut self, name: impl Into<String>) -> Self {
        self.lex = Some(name.into());
        self
    }

    /// Set the parse engine name (builder style).
    pub fn with_parse(mut self, name: impl Into<String>) -> Self {
        self.parse = Some(name.into());
        self
    }

    /// Set the eval engine name (builder style).
    pub fn with_eval(mut self, name: impl Into<String>) -> Self {
        self.eval = Some(name.into());
        self
    }
}

/// A dyn-compatible wrapper around [`ParserEngine`].
///
/// [`ParserEngine`] is not dyn-compatible because its `expression` method
/// takes a [`ParserContext`] parameterized by `Self`. This wrapper exposes
/// only the [`parse`](ParserEngine::parse) method, which is dyn-compatible,
/// allowing parse engines to be stored as `Box<dyn ParseEngineDyn>`.
///
/// Any type implementing [`ParserEngine`] automatically implements this trait
/// via a blanket impl.
pub trait ParseEngineDyn {
    /// Parse a token stream into an [`Ast`] using the given language.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on parse failure.
    fn parse_dyn(&self, lang: &Language, req: &ParserRequest) -> Result<Ast, LangError>;
}

impl<E: ParserEngine> ParseEngineDyn for E {
    fn parse_dyn(&self, lang: &Language, req: &ParserRequest) -> Result<Ast, LangError> {
        ParserEngine::parse(self, lang, req)
    }
}

/// Binds a [`Language`] to engine impls and exposes staged `lex`/`parse`/`eval`
/// (ADR-0044 §3).
///
/// A `Runtime` owns the [`Language`] and optional engine trait objects for each
/// stage. Each stage method ([`lex`](Runtime::lex),
/// [`parse`](Runtime::parse), [`eval`](Runtime::eval)) dispatches to its bound
/// engine or returns [`LangError::NotImplemented`] if no engine is attached.
///
/// The lifetime `'h` ties the runtime to any borrowed host-functions registry
/// (used by eval engines that resolve [`Scheme::Foreign`] calls). An eval-only
/// runtime — no lex or parse engine — is fully supported.
///
/// # Host-function plumbing
///
/// When constructed via [`Runtime::from_config`] with `host_functions =
/// Some(host)`, the host registry is passed to the eval engine (e.g. via
/// [`TreeWalkEngine::with_host`]). When an eval engine is set manually via
/// [`Runtime::with_eval_engine`], **the caller is responsible for wiring host
/// functions into that engine directly** — the `Runtime` does not rewire a
/// manually-set engine.
pub struct Runtime<'h> {
    /// The language definition bound to this runtime.
    lang: Language,
    /// The optional lex engine.
    lex_engine: Option<Box<dyn LexerEngine>>,
    /// The optional parse engine (wrapped for dyn compatibility).
    parse_engine: Option<Box<dyn ParseEngineDyn>>,
    /// The optional eval engine. The `'h` lifetime allows storing engines that
    /// borrow a host-functions registry (e.g. `TreeWalkEngine<'h>`).
    eval_engine: Option<Box<dyn EvalEngine + 'h>>,
    /// The optional host-functions registry, retained for inspection and for
    /// `from_config` to pass to the eval engine during construction.
    host_functions: Option<&'h dyn HostFunctions>,
}

impl<'h> std::fmt::Debug for Runtime<'h> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("lang", &self.lang)
            .field("has_lex_engine", &self.lex_engine.is_some())
            .field("has_parse_engine", &self.parse_engine.is_some())
            .field("has_eval_engine", &self.eval_engine.is_some())
            .field("has_host_functions", &self.host_functions.is_some())
            .finish()
    }
}

impl<'h> Runtime<'h> {
    /// Create a new runtime bound to `lang` with no engines attached.
    ///
    /// All stage methods (`lex`, `parse`, `eval`) will return
    /// [`LangError::NotImplemented`] until engines are attached via the
    /// `with_*` builders or [`Runtime::from_config`].
    pub fn new(lang: Language) -> Self {
        Self {
            lang,
            lex_engine: None,
            parse_engine: None,
            eval_engine: None,
            host_functions: None,
        }
    }

    /// Attach a lex engine (builder style).
    pub fn with_lex_engine(mut self, engine: Box<dyn LexerEngine>) -> Self {
        self.lex_engine = Some(engine);
        self
    }

    /// Attach a parse engine (builder style).
    ///
    /// Accepts any type implementing [`ParserEngine`]; the engine is stored
    /// via the dyn-compatible [`ParseEngineDyn`] wrapper.
    pub fn with_parse_engine<E: ParserEngine + 'static>(mut self, engine: E) -> Self {
        self.parse_engine = Some(Box::new(engine));
        self
    }

    /// Attach an eval engine (builder style).
    ///
    /// The caller is responsible for wiring host functions into this engine
    /// directly (e.g. calling `TreeWalkEngine::with_host` before boxing). The
    /// `Runtime` does not rewire a manually-set engine.
    pub fn with_eval_engine(mut self, engine: Box<dyn EvalEngine + 'h>) -> Self {
        self.eval_engine = Some(engine);
        self
    }

    /// Record a host-functions registry (builder style).
    ///
    /// This stores the registry for inspection but does **not** rewire an
    /// already-attached eval engine. Use [`Runtime::from_config`] to have the
    /// runtime wire host functions into the eval engine automatically.
    pub fn with_host_functions(mut self, host: &'h dyn HostFunctions) -> Self {
        self.host_functions = Some(host);
        self
    }

    /// The language definition bound to this runtime.
    pub fn lang(&self) -> &Language {
        &self.lang
    }

    /// The host-functions registry, if one was provided.
    pub fn host_functions(&self) -> Option<&'h dyn HostFunctions> {
        self.host_functions
    }

    /// Lex source text into a token stream using the bound lex engine.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::NotImplemented`] if no lex engine is bound, or
    /// propagates the engine's error.
    pub fn lex(&self, src: &Source) -> Result<Tokens, LangError> {
        let engine = self.lex_engine.as_ref().ok_or(LangError::NotImplemented)?;
        let req = LexerRequest::new(src.clone());
        engine.lex(&self.lang, &req)
    }

    /// Parse a token stream into an [`Ast`] using the bound parse engine.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::NotImplemented`] if no parse engine is bound, or
    /// propagates the engine's error.
    pub fn parse(&self, toks: &Tokens) -> Result<Ast, LangError> {
        let engine = self
            .parse_engine
            .as_ref()
            .ok_or(LangError::NotImplemented)?;
        let req = ParserRequest::new(toks.clone());
        engine.parse_dyn(&self.lang, &req)
    }

    /// Evaluate an [`Ast`] into a [`Value`] using the bound eval engine.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::NotImplemented`] if no eval engine is bound, or
    /// propagates the engine's error.
    pub fn eval(&self, ast: &Ast) -> Result<Value, LangError> {
        let engine = self
            .eval_engine
            .as_ref()
            .ok_or(LangError::NotImplemented)?;
        let req = EvalRequest::new(ast.clone());
        engine.eval(&self.lang, &req)
    }

    /// Convenience: `eval(parse(lex(src)?)?)`.
    ///
    /// Composes all three stages. If any stage's engine is missing, returns
    /// [`LangError::NotImplemented`] from that stage.
    ///
    /// # Errors
    ///
    /// Propagates the first stage error.
    pub fn run(&self, src: &Source) -> Result<Value, LangError> {
        let toks = self.lex(src)?;
        let ast = self.parse(&toks)?;
        self.eval(&ast)
    }

    /// Build a [`Runtime`] from a [`Language`] and a data-driven
    /// [`RuntimeConfig`] (ADR-0044 §7).
    ///
    /// Resolves engine names to concrete engine crates at build time. The
    /// **selection** is data; the resolution is a direct dependency. If
    /// `host_functions` is `Some`, it is passed to the eval engine (e.g. via
    /// [`TreeWalkEngine::with_host`]).
    ///
    /// # Phase 3 resolvable names
    ///
    /// - `parse = "pratt"` → [`PrattEngine`]
    /// - `eval = "tree-walk"` → [`TreeWalkEngine`] (with host if provided,
    ///   plus any globals injected as root-scope variables)
    /// - `eval = "bytecode"` → [`BytecodeEngine`] (with host if provided;
    ///   globals are not yet supported by the bytecode engine)
    /// - `eval = "cranelift"` → [`CraneliftEngine`] (with host if provided;
    ///   globals are not yet supported by the cranelift engine)
    /// - `lex = "data"` → [`DataLexerEngine`] (data-driven lexer reading
    ///   `LexerRuleDef` fragments)
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Engine`] for any unknown engine name.
    pub fn from_config(
        lang: Language,
        config: &RuntimeConfig,
        host_functions: Option<&'h dyn HostFunctions>,
    ) -> Result<Self, LangError> {
        Self::from_config_with_globals(lang, config, host_functions, None)
    }

    /// Like [`from_config`] but injects `globals` as root-scope variables
    /// in the eval engine (e.g. `self` → language value). ADR-0041 Phase 3.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Engine`] for any unknown engine name.
    pub fn from_config_with_globals(
        lang: Language,
        config: &RuntimeConfig,
        host_functions: Option<&'h dyn HostFunctions>,
        globals: Option<&HashMap<String, Value>>,
    ) -> Result<Self, LangError> {
        // Resolve lex engine.
        let lex_engine: Option<Box<dyn LexerEngine>> = match &config.lex {
            Some(name) => match name.as_str() {
                "data" => Some(Box::new(DataLexerEngine::new())),
                _ => {
                    return Err(LangError::Engine(format!(
                        "unknown lex engine: {}",
                        name
                    )));
                }
            },
            None => None,
        };

        // Resolve parse engine.
        let parse_engine: Option<Box<dyn ParseEngineDyn>> = match &config.parse {
            Some(name) => match name.as_str() {
                "pratt" => Some(Box::new(PrattEngine::new())),
                _ => {
                    return Err(LangError::Engine(format!(
                        "unknown parse engine: {}",
                        name
                    )));
                }
            },
            None => None,
        };

        // Resolve eval engine.
        let eval_engine: Option<Box<dyn EvalEngine + 'h>> = match &config.eval {
            Some(name) => match name.as_str() {
                "tree-walk" => {
                    let mut engine = match host_functions {
                        Some(host) => TreeWalkEngine::new().with_host(host),
                        None => TreeWalkEngine::new(),
                    };
                    if let Some(globals) = globals {
                        for (name, val) in globals {
                            engine = engine.with_global(name, val.clone());
                        }
                    }
                    Some(Box::new(engine))
                }
                "bytecode" => {
                    // BytecodeEngine supports host functions via `with_host`,
                    // but does not yet expose a `with_global` builder, so
                    // globals are not injected into the bytecode engine.
                    let engine = match host_functions {
                        Some(host) => BytecodeEngine::with_host(host),
                        None => BytecodeEngine::new(),
                    };
                    Some(Box::new(engine))
                }
                "cranelift" => {
                    // CraneliftEngine supports host functions via `with_host`,
                    // but does not yet expose a `with_global` builder, so
                    // globals are not injected into the cranelift engine.
                    let engine = match host_functions {
                        Some(host) => CraneliftEngine::with_host(host),
                        None => CraneliftEngine::new(),
                    };
                    Some(Box::new(engine))
                }
                _ => {
                    return Err(LangError::Engine(format!(
                        "unknown eval engine: {}",
                        name
                    )));
                }
            },
            None => None,
        };

        Ok(Self {
            lang,
            lex_engine,
            parse_engine,
            eval_engine,
            host_functions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{
        AstNode, AstShapeRef, EvalSchemeDef, GrammarPosition, GrammarRow, Scheme, Token,
        TokenKindDef,
    };
    use handl_plugin_engine_eval::HostFn;
    use handl_plugin_engine_lex::Lexer;
    use handl_plugin_engine_parse::{Parser, ParserContext};
    use serde_json::json;
    use std::collections::HashMap;

    // -----------------------------------------------------------------------
    // Stub engines for independent dispatch testing
    // -----------------------------------------------------------------------

    /// A stub lex engine that returns a fixed token stream.
    struct StubLex {
        tokens: Tokens,
    }

    impl LexerEngine for StubLex {
        fn lex(
            &self,
            _lang: &Language,
            _req: &LexerRequest,
        ) -> Result<Tokens, LangError> {
            Ok(self.tokens.clone())
        }
    }

    /// A stub parse engine that returns a single-node Ast.
    struct StubParse;

    impl ParserEngine for StubParse {
        fn expression(
            &self,
            _ctx: &mut ParserContext<'_, '_, Self>,
            _rbp: u8,
        ) -> Result<handl_language::AstRef, LangError> {
            Ok(handl_language::AstRef::new(0))
        }

        fn parse(
            &self,
            _lang: &Language,
            req: &ParserRequest,
        ) -> Result<Ast, LangError> {
            let mut ast = Ast::new();
            if !req.tokens.as_slice().is_empty() {
                ast.push(
                    AstNode::new("stub")
                        .with_payload(json!(req.tokens.as_slice()[0].text.clone())),
                );
            }
            Ok(ast)
        }
    }

    /// A stub eval engine that returns a fixed value.
    struct StubEval {
        value: Value,
    }

    impl EvalEngine for StubEval {
        fn eval(&self, _lang: &Language, _req: &EvalRequest) -> Result<Value, LangError> {
            Ok(self.value.clone())
        }
    }

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    /// A simple host function registry for tests.
    struct MapHost {
        fns: HashMap<String, Box<dyn HostFn>>,
    }

    impl MapHost {
        fn new() -> Self {
            Self {
                fns: HashMap::new(),
            }
        }
    }

    impl HostFunctions for MapHost {
        fn lookup(&self, name: &str) -> Option<&dyn HostFn> {
            self.fns.get(name).map(|f| f.as_ref())
        }
    }

    /// Build a small arithmetic language: numbers + `+` infix, with eval
    /// schemes for `lit` (no scheme → payload) and `binary.add` (Prim add).
    fn arith_lang() -> Language {
        let mut lang = Language::new();
        // Token kinds
        lang.tokens.merge(TokenKindDef::new("token.number")).unwrap();
        lang.tokens.merge(TokenKindDef::new("token.plus")).unwrap();
        // Grammar: number prefix (atom, arity 0)
        lang.grammar
            .merge(GrammarRow::new(
                "g.num",
                "token.number",
                GrammarPosition::Prefix,
                AstShapeRef::new("lit", 0),
            ))
            .unwrap();
        // Grammar: + infix (lbp 10, rbp 11)
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.plus",
                    "token.plus",
                    GrammarPosition::Infix,
                    AstShapeRef::new("binary.add", 2),
                )
                .with_binding_power(10, 11),
            )
            .unwrap();
        // Eval scheme: binary.add → Prim { op: "add", args: [Child(0), Child(1)] }
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        lang
    }

    fn tok(kind: &str, text: &str) -> Token {
        Token {
            kind: kind.into(),
            text: text.into(),
        }
    }

    // -----------------------------------------------------------------------
    // Runtime construction with engine selection (each independently)
    // -----------------------------------------------------------------------

    #[test]
    fn new_runtime_has_no_engines() {
        let lang = Language::new();
        let rt = Runtime::new(lang);
        assert_eq!(rt.lex(&Source::new("x")).unwrap_err(), LangError::NotImplemented);
        assert_eq!(rt.parse(&Tokens::new()).unwrap_err(), LangError::NotImplemented);
        assert_eq!(rt.eval(&Ast::new()).unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn with_lex_engine_attaches_lex_only() {
        let rt = Runtime::new(Language::new()).with_lex_engine(Box::new(StubLex {
            tokens: Tokens(vec![tok("token.number", "42")]),
        }));
        // lex works, parse/eval still NotImplemented.
        let toks = rt.lex(&Source::new("42")).unwrap();
        assert_eq!(toks.as_slice().len(), 1);
        assert_eq!(rt.parse(&toks).unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn with_parse_engine_attaches_parse_only() {
        let rt = Runtime::new(Language::new()).with_parse_engine(StubParse);
        let toks = Tokens(vec![tok("token.number", "42")]);
        let ast = rt.parse(&toks).unwrap();
        assert_eq!(ast.len(), 1);
        assert_eq!(ast.nodes()[0].kind, "stub");
        // lex/eval still NotImplemented.
        assert_eq!(rt.lex(&Source::new("x")).unwrap_err(), LangError::NotImplemented);
        assert_eq!(rt.eval(&ast).unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn with_eval_engine_attaches_eval_only() {
        let rt = Runtime::new(Language::new()).with_eval_engine(Box::new(StubEval {
            value: json!(99),
        }));
        let result = rt.eval(&Ast::new()).unwrap();
        assert_eq!(result, json!(99));
        // lex/parse still NotImplemented.
        assert_eq!(rt.lex(&Source::new("x")).unwrap_err(), LangError::NotImplemented);
        assert_eq!(rt.parse(&Tokens::new()).unwrap_err(), LangError::NotImplemented);
    }

    // -----------------------------------------------------------------------
    // Runtime::lex/parse/eval dispatch to bound engines
    // -----------------------------------------------------------------------

    #[test]
    fn lex_dispatches_to_bound_engine() {
        let expected = Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]);
        let rt = Runtime::new(Language::new()).with_lex_engine(Box::new(StubLex {
            tokens: expected.clone(),
        }));
        let got = rt.lex(&Source::new("1 + 2")).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn parse_dispatches_to_bound_engine() {
        let rt = Runtime::new(Language::new()).with_parse_engine(StubParse);
        let toks = Tokens(vec![tok("token.number", "7")]);
        let ast = rt.parse(&toks).unwrap();
        assert_eq!(ast.nodes()[0].payload, json!("7"));
    }

    #[test]
    fn eval_dispatches_to_bound_engine() {
        let rt = Runtime::new(Language::new()).with_eval_engine(Box::new(StubEval {
            value: json!("hello"),
        }));
        let val = rt.eval(&Ast::new()).unwrap();
        assert_eq!(val, json!("hello"));
    }

    // -----------------------------------------------------------------------
    // Runtime with a missing engine (eval-only runtime)
    // -----------------------------------------------------------------------

    #[test]
    fn eval_only_runtime_lex_returns_error_eval_works() {
        let rt = Runtime::new(arith_lang())
            .with_eval_engine(Box::new(TreeWalkEngine::new()));
        // lex returns NotImplemented (no lex engine).
        assert_eq!(rt.lex(&Source::new("1 + 2")).unwrap_err(), LangError::NotImplemented);
        // parse returns NotImplemented (no parse engine).
        assert_eq!(rt.parse(&Tokens::new()).unwrap_err(), LangError::NotImplemented);
        // eval works on a pre-parsed Ast.
        // Ast: 0: lit(2), 1: lit(3), 2: binary.add {children: [0, 1]}
        let mut ast = Ast::new();
        ast.push(AstNode::new("lit").with_payload(json!(2)));
        ast.push(AstNode::new("lit").with_payload(json!(3)));
        ast.push(AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})));
        let result = rt.eval(&ast).unwrap();
        assert_eq!(result, json!(5));
    }

    // -----------------------------------------------------------------------
    // Engine selection from data (RuntimeConfig)
    // -----------------------------------------------------------------------

    #[test]
    fn from_config_eval_tree_walk_binds_tree_walk() {
        let lang = arith_lang();
        let config = RuntimeConfig::new().with_eval("tree-walk");
        let rt = Runtime::from_config(lang, &config, None).unwrap();
        // eval should work (not NotImplemented).
        let mut ast = Ast::new();
        ast.push(AstNode::new("lit").with_payload(json!(42)));
        let result = rt.eval(&ast).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn from_config_eval_bytecode_binds_bytecode() {
        let lang = Language::new();
        let config = RuntimeConfig::new().with_eval("bytecode");
        let rt = Runtime::from_config(lang, &config, None).unwrap();
        // eval should work (not NotImplemented); an empty Ast returns Null.
        let result = rt.eval(&Ast::new()).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn from_config_eval_cranelift_binds_cranelift() {
        let lang = Language::new();
        let config = RuntimeConfig::new().with_eval("cranelift");
        let rt = Runtime::from_config(lang, &config, None).unwrap();
        // eval should work (not NotImplemented); an empty Ast returns Null.
        let result = rt.eval(&Ast::new()).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn from_config_unknown_eval_name_returns_config_error() {
        let lang = Language::new();
        let config = RuntimeConfig::new().with_eval("nonsense");
        let err = Runtime::from_config(lang, &config, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
        assert_eq!(err, LangError::Engine("unknown eval engine: nonsense".into()));
    }

    #[test]
    fn from_config_parse_pratt_binds_pratt() {
        let lang = arith_lang();
        let config = RuntimeConfig::new().with_parse("pratt");
        let rt = Runtime::from_config(lang, &config, None).unwrap();
        // parse should work (not NotImplemented).
        let toks = Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]);
        let ast = rt.parse(&toks).unwrap();
        assert_eq!(ast.len(), 3);
        assert_eq!(ast.nodes()[2].kind, "binary.add");
    }

    #[test]
    fn from_config_unknown_parse_name_returns_config_error() {
        let lang = Language::new();
        let config = RuntimeConfig::new().with_parse("chumsky");
        let err = Runtime::from_config(lang, &config, None).unwrap_err();
        assert_eq!(err, LangError::Engine("unknown parse engine: chumsky".into()));
    }

    #[test]
    fn from_config_lex_any_name_returns_config_error() {
        // No lex engine crate exists in Phase 3.
        let lang = Language::new();
        let config = RuntimeConfig::new().with_lex("pratt");
        let err = Runtime::from_config(lang, &config, None).unwrap_err();
        assert_eq!(err, LangError::Engine("unknown lex engine: pratt".into()));
    }

    #[test]
    fn from_config_lex_logos_returns_config_error() {
        let lang = Language::new();
        let config = RuntimeConfig::new().with_lex("logos");
        let err = Runtime::from_config(lang, &config, None).unwrap_err();
        assert_eq!(err, LangError::Engine("unknown lex engine: logos".into()));
    }

    #[test]
    fn from_config_lex_pattern_binds_pattern_lexer() {
        // The "data" lex engine name binds the data-driven DataLexerEngine.
        let lang = Language::new();
        let config = RuntimeConfig::new().with_lex("data");
        let rt = Runtime::from_config(lang, &config, None).unwrap();
        // lex is bound (no NotImplemented); an empty language has no rules so
        // lexing any non-empty source yields a LexError, not NotImplemented.
        let err = rt.lex(&Source::new("x")).unwrap_err();
        assert!(matches!(err, LangError::LexError { .. }));
    }

    #[test]
    fn from_config_empty_config_leaves_all_unbound() {
        let lang = Language::new();
        let config = RuntimeConfig::new();
        let rt = Runtime::from_config(lang, &config, None).unwrap();
        assert_eq!(rt.lex(&Source::new("x")).unwrap_err(), LangError::NotImplemented);
        assert_eq!(rt.parse(&Tokens::new()).unwrap_err(), LangError::NotImplemented);
        assert_eq!(rt.eval(&Ast::new()).unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn from_config_with_host_functions_passes_to_eval_engine() {
        // A host function that doubles its integer argument.
        struct DoubleFn;
        impl HostFn for DoubleFn {
            fn call(&self, args: &[Value]) -> Result<Value, LangError> {
                Ok(Value::from(args[0].as_i64().unwrap() * 2))
            }
        }
        let mut host = MapHost::new();
        host.fns.insert("double".into(), Box::new(DoubleFn));

        // Build a language with a "foreign" node kind that calls "double".
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.dbl",
                "foreign.double",
                Scheme::foreign(
                    "double",
                    vec![Scheme::Child(handl_language::AstRef::new(0))],
                ),
            ))
            .unwrap();

        let config = RuntimeConfig::new().with_eval("tree-walk");
        let rt = Runtime::from_config(lang, &config, Some(&host)).unwrap();
        assert!(rt.host_functions().is_some());

        // Ast: 0: lit(21), 1: foreign.double {children: [0]}
        let mut ast = Ast::new();
        ast.push(AstNode::new("lit").with_payload(json!(21)));
        ast.push(
            AstNode::new("foreign.double").with_payload(json!({"children": [0]})),
        );
        let result = rt.eval(&ast).unwrap();
        assert_eq!(result, json!(42));
    }

    // -----------------------------------------------------------------------
    // Runtime::run convenience composes the three stages
    // -----------------------------------------------------------------------

    #[test]
    fn run_composes_three_stages_with_stub_engines() {
        let stub_tokens = Tokens(vec![tok("token.number", "42")]);
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: stub_tokens,
            }))
            .with_parse_engine(StubParse)
            .with_eval_engine(Box::new(StubEval {
                value: json!(123),
            }));
        let result = rt.run(&Source::new("42")).unwrap();
        assert_eq!(result, json!(123));
    }

    #[test]
    fn run_without_lex_engine_returns_not_implemented() {
        let rt = Runtime::new(Language::new())
            .with_parse_engine(StubParse)
            .with_eval_engine(Box::new(StubEval {
                value: json!(0),
            }));
        assert_eq!(rt.run(&Source::new("x")).unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn run_without_parse_engine_returns_not_implemented() {
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: Tokens(vec![tok("token.number", "1")]),
            }))
            .with_eval_engine(Box::new(StubEval {
                value: json!(0),
            }));
        // lex succeeds, parse returns NotImplemented.
        assert_eq!(rt.run(&Source::new("1")).unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn run_without_eval_engine_returns_not_implemented() {
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: Tokens(vec![tok("token.number", "1")]),
            }))
            .with_parse_engine(StubParse);
        // lex and parse succeed, eval returns NotImplemented.
        assert_eq!(rt.run(&Source::new("1")).unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn run_with_no_engines_returns_not_implemented() {
        let rt = Runtime::new(Language::new());
        assert_eq!(rt.run(&Source::new("x")).unwrap_err(), LangError::NotImplemented);
    }

    // -----------------------------------------------------------------------
    // RuntimeConfig serde round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn runtime_config_serde_roundtrip() {
        let config = RuntimeConfig::new()
            .with_lex("logos")
            .with_parse("pratt")
            .with_eval("tree-walk");
        let json_str = serde_json::to_string(&config).unwrap();
        let back: RuntimeConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back, config);
    }

    #[test]
    fn runtime_config_empty_serde_roundtrip() {
        let config = RuntimeConfig::new();
        let json_str = serde_json::to_string(&config).unwrap();
        let back: RuntimeConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back, config);
        assert!(back.lex.is_none());
        assert!(back.parse.is_none());
        assert!(back.eval.is_none());
    }

    #[test]
    fn runtime_config_partial_serde_roundtrip() {
        let config = RuntimeConfig::new().with_eval("tree-walk");
        let json_str = serde_json::to_string(&config).unwrap();
        let back: RuntimeConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back, config);
        assert!(back.lex.is_none());
        assert!(back.parse.is_none());
        assert_eq!(back.eval.as_deref(), Some("tree-walk"));
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    #[test]
    fn lang_accessor_returns_bound_language() {
        let lang = arith_lang();
        let rt = Runtime::new(lang.clone());
        assert_eq!(rt.lang(), &lang);
    }

    #[test]
    fn host_functions_accessor_returns_none_by_default() {
        let rt = Runtime::new(Language::new());
        assert!(rt.host_functions().is_none());
    }

    #[test]
    fn host_functions_accessor_returns_some_when_set() {
        let host = MapHost::new();
        let rt = Runtime::new(Language::new()).with_host_functions(&host);
        assert!(rt.host_functions().is_some());
    }

    // -----------------------------------------------------------------------
    // Builder chaining
    // -----------------------------------------------------------------------

    #[test]
    fn builder_chains_all_engines() {
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: Tokens(vec![tok("token.number", "1")]),
            }))
            .with_parse_engine(StubParse)
            .with_eval_engine(Box::new(StubEval {
                value: json!(42),
            }));
        let result = rt.run(&Source::new("1")).unwrap();
        assert_eq!(result, json!(42));
    }

    // -----------------------------------------------------------------------
    // Operation markers (verify re-exports compile)
    // -----------------------------------------------------------------------

    #[test]
    fn operation_markers_are_accessible() {
        let _ = Lexer;
        let _ = Parser;
        let _ = handl_plugin_engine_eval::Eval;
    }

    // -----------------------------------------------------------------------
    // Debug impl
    // -----------------------------------------------------------------------

    #[test]
    fn runtime_debug_impl_works() {
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: Tokens::new(),
            }))
            .with_parse_engine(StubParse)
            .with_eval_engine(Box::new(StubEval {
                value: json!(0),
            }));
        let debug_str = format!("{:?}", rt);
        assert!(debug_str.contains("Runtime"));
        assert!(debug_str.contains("has_lex_engine: true"));
        assert!(debug_str.contains("has_parse_engine: true"));
        assert!(debug_str.contains("has_eval_engine: true"));
    }

    #[test]
    fn runtime_debug_no_engines() {
        let rt = Runtime::new(Language::new());
        let debug_str = format!("{:?}", rt);
        assert!(debug_str.contains("has_lex_engine: false"));
        assert!(debug_str.contains("has_parse_engine: false"));
        assert!(debug_str.contains("has_eval_engine: false"));
    }

    // -----------------------------------------------------------------------
    // StubParse::expression coverage (required by trait, not called via parse)
    // -----------------------------------------------------------------------

    #[test]
    fn stub_parse_expression_is_callable() {
        // StubParse overrides parse(), so its expression() method is never
        // called through the normal dispatch path. Call it directly to
        // verify the trait impl is correct and cover those lines.
        use handl_plugin_engine_parse::ParserEngine as _;
        let engine = StubParse;
        let lang = Language::new();
        let toks = vec![Token {
            kind: "token.x".into(),
            text: "x".into(),
        }];
        let mut ctx = ParserContext::new(&engine, &lang, &toks);
        let r = engine.expression(&mut ctx, 0).unwrap();
        assert_eq!(r.index(), 0);
    }
}
