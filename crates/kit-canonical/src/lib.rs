//! Canonical kit as a generic plugin host.
//!
//! `Kit` loads a [`KitManifest`], registers the core canonical plugins, and
//! composes a [`Lexer`], a parser, and a node factory from them. It no longer
//! depends on the concrete `handl-ast-canonical`, `handl-lexer-canonical`, or
//! `handl-parser-canonical` crates.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_plugin_lexer::{
    DefaultLexError, Lexer, Token,
};
use handl_plugin_grammar_textmate::{GrammarContributor, GrammarRule};
pub use handl_plugin_token_kind::{
    DynFactory, DynKind, DynLexer, DynTemplateSegment,
};

use handl_platform::{
    AstKindDeclaration, AstKindRole, AstNode, BasicEvaluator, BasicTypechecker,
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    OptionsPlugin, PlatformError, Plugin, PluginContext, PluginManifest, RuleAcceptor,
};
use handl_plugin_diag_expression::ExpressionDiagnostics;
use handl_plugin_ext_build::BasicBuilder;
use handl_plugin_ext_compile::BasicCompiler;
use handl_plugin_ext_fmt::BasicFormatter;
use handl_plugin_ext_lexer::BasicLexer;
use handl_plugin_ext_lint::BasicLinter;
use handl_plugin_ext_parser::{BasicParser, ParserBehavior};
use handl_plugin_fmt_stmt::StatementFormatter;
use handl_plugin_fmt_trivia::TriviaFormatter;

use handl_plugin_load::LoaderHostPlugin;
use handl_plugin_loader_native::NativeLoader;
use handl_plugin_loader_wasm::WasmLoader;

use handl_plugin_ast_apply::Apply;
use handl_plugin_ast_array::Array;
use handl_plugin_ast_assign::Assign;
use handl_plugin_ast_call::Call;
use handl_plugin_ast_handle::Handle;
use handl_plugin_ast_index::Index;
use handl_plugin_ast_let::Let;
use handl_plugin_ast_match::{Match, MatchArm};
use handl_plugin_ast_object::Object;
use handl_plugin_ast_pipe::Pipe;
use handl_plugin_ast_var::Var;
pub use handl_plugin_ast_handl_value::KitValue;
pub use handl_plugin_ast_stmt_export::Export;
pub use handl_plugin_ast_stmt_import::Import;
pub use handl_plugin_ast_stmt_module::Module;

// Re-export the AST composition layer (KitNode + converters) that now lives in
// the dedicated `handl-plugin-ast-handl` crate.
pub use handl_plugin_ast_handl::{ast_to_kitnode, KitMetadata, KitNode};
use handl_plugin_eval_hello::HelloEvalPlugin;
use handl_plugin_eval_json::JsonEvalPlugin;
use handl_plugin_grammar_apply::ApplyRule;
use handl_plugin_grammar_array::ArrayRule;
use handl_plugin_grammar_assign::AssignRule;
use handl_plugin_grammar_call::CallRule;
use handl_plugin_grammar_function_literal::ArrowRule;
use handl_plugin_grammar_handle::HandleRule;
use handl_plugin_grammar_index::IndexRule;
use handl_plugin_grammar_match::MatchRule;
use handl_plugin_grammar_object::ObjectRule;
use handl_plugin_grammar_pipe::PipeRule;
use handl_plugin_grammar_primary_identifier::IdentifierRule;
use handl_plugin_grammar_primary_value::ValueRule;
use handl_plugin_grammar_stmt_export::ExportStmtRule;
use handl_plugin_grammar_stmt_import::ImportStmtRule;
use handl_plugin_grammar_stmt_module::ModuleStmtRule;
use handl_plugin_grammar_template::TemplateRule;
use handl_plugin_grammar_type_annot::TypeAnnotRule;
use handl_plugin_target_native::NativeTarget;
// New eval/exec/module plugins (ADR 0040 steps 3-8).
use handl_plugin_eval_array::EvalArrayPlugin;
use handl_plugin_eval_assign::EvalAssignPlugin;
use handl_plugin_eval_call::EvalCallPlugin;
use handl_plugin_eval_core::EvalCorePlugin;
use handl_plugin_eval_field_access::EvalFieldAccessPlugin;
use handl_plugin_eval_function::EvalFunctionPlugin;
use handl_plugin_eval_handle::EvalHandlePlugin;
use handl_plugin_eval_index::EvalIndexPlugin;
use handl_plugin_eval_object::EvalObjectPlugin;
use handl_plugin_eval_value::EvalValuePlugin;
use handl_plugin_exec_max_calls::ExecMaxCallsPlugin;
use handl_plugin_exec_max_depth::ExecMaxDepthPlugin;
use handl_plugin_module::ModuleRuntime;

pub use handl_plugin_diag_core::{Diagnostic, DiagnosticLevel, Diagnostics};

pub mod kit_manifest;
pub use kit_manifest::{KitManifest, canonical_manifest};

/// Like [`ast_to_kitnode`] but with access to the [`Kit`] for recursive
/// parsing of template interpolation expressions.
fn ast_to_kitnode_with_kit(ast: &AstNode, kit: &Kit) -> Result<KitNode, PlatformError> {
    match ast.kind.as_str() {
        "template" => {
            let segments = ast
                .payload
                .get("segments")
                .and_then(|v| v.as_array())
                .ok_or_else(|| {
                    PlatformError::Plugin("missing 'segments' field in template AST payload".into())
                })?;

            let mut parts: Vec<KitNode> = Vec::new();
            for seg in segments {
                let seg_type = seg.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
                    PlatformError::Plugin("missing 'type' in template segment".into())
                })?;

                match seg_type {
                    "text" => {
                        let text = seg.get("value").and_then(|v| v.as_str()).ok_or_else(|| {
                            PlatformError::Plugin("missing 'value' in text segment".into())
                        })?;
                        parts.push(KitNode::Literal(KitValue::String(text.to_string())));
                    }
                    "expr" => {
                        let raw = seg.get("raw").and_then(|v| v.as_str()).ok_or_else(|| {
                            PlatformError::Plugin("missing 'raw' in expr segment".into())
                        })?;
                        let expr_node = kit.parse(raw)?;
                        parts.push(expr_node);
                    }
                    _ => {
                        return Err(PlatformError::Plugin(format!(
                            "unknown template segment type '{}'",
                            seg_type
                        )));
                    }
                }
            }

            if parts.is_empty() {
                Ok(KitNode::Literal(KitValue::String(String::new())))
            } else if parts.len() == 1 {
                Ok(parts.into_iter().next().unwrap())
            } else {
                let mut iter = parts.into_iter().rev();
                let mut acc = iter.next().unwrap();
                for part in iter {
                    acc = KitNode::Call(Call::new("str.concat", vec![part, acc]));
                }
                Ok(acc)
            }
        }
        _ => ast_to_kitnode(ast),
    }
}

/// The set of grammar-rule plugin ids that the canonical kit knows how to
/// wire into a `BasicParser<DynKind>`.
const GRAMMAR_RULE_PLUGIN_IDS: &[&str] = &[
    "plugin-grammar-primary-value",
    "plugin-grammar-primary-identifier",
    "plugin-grammar-call",
    "plugin-grammar-index",
    "plugin-grammar-pipe",
    "plugin-grammar-function-literal",
    "plugin-grammar-template",
    "plugin-grammar-apply",
    "plugin-grammar-assign",
    "plugin-grammar-type-annot",
    "plugin-grammar-array",
    "plugin-grammar-object",
    "plugin-grammar-match",
    "plugin-grammar-handle",
    "plugin-grammar-stmt-import",
    "plugin-grammar-stmt-export",
    "plugin-grammar-stmt-module",
];

/// Returns a freshly-constructed rule for the given grammar-rule plugin id, or
/// `None` if the id is not a recognized grammar rule.
fn rule_for_plugin_id(id: &str) -> Option<Box<dyn ParserBehavior<DynKind>>> {
    let rule: Box<dyn ParserBehavior<DynKind>> = match id {
        "plugin-grammar-call" => Box::new(CallRule),
        "plugin-grammar-apply" => Box::new(ApplyRule),
        "plugin-grammar-index" => Box::new(IndexRule),
        "plugin-grammar-pipe" => Box::new(PipeRule),
        "plugin-grammar-function-literal" => Box::new(ArrowRule),
        "plugin-grammar-template" => Box::new(TemplateRule),
        "plugin-grammar-assign" => Box::new(AssignRule),
        "plugin-grammar-array" => Box::new(ArrayRule),
        "plugin-grammar-object" => Box::new(ObjectRule),
        "plugin-grammar-match" => Box::new(MatchRule),
        "plugin-grammar-handle" => Box::new(HandleRule),
        "plugin-grammar-primary-identifier" => Box::new(IdentifierRule),
        "plugin-grammar-primary-value" => Box::new(ValueRule),
        "plugin-grammar-type-annot" => Box::new(TypeAnnotRule),
        "plugin-grammar-stmt-import" => Box::new(ImportStmtRule),
        "plugin-grammar-stmt-export" => Box::new(ExportStmtRule),
        "plugin-grammar-stmt-module" => Box::new(ModuleStmtRule),
        _ => return None,
    };
    Some(rule)
}

/// Builds a parser populated with the grammar rules for the given plugin ids.
fn parser_for_plugins(ids: &[String]) -> Result<BasicParser<DynKind>, PlatformError> {
    let mut parser = BasicParser::<DynKind>::new();
    for id in ids {
        if let Some(rule) = rule_for_plugin_id(id) {
            parser.accept(rule)?;
        }
    }
    Ok(parser)
}

const AST_KIND_TABLE: &[(&str, &str, AstKindRole)] = &[
    ("plugin-grammar-primary-value", "value", AstKindRole::Produces),
    ("plugin-grammar-primary-identifier", "var", AstKindRole::Produces),
    ("plugin-grammar-call", "call", AstKindRole::Produces),
    ("plugin-grammar-apply", "apply", AstKindRole::Produces),
    ("plugin-grammar-index", "index", AstKindRole::Produces),
    ("plugin-grammar-pipe", "pipe", AstKindRole::Produces),
    ("plugin-grammar-function-literal", "function", AstKindRole::Produces),
    ("plugin-grammar-assign", "assign", AstKindRole::Produces),
    ("plugin-grammar-array", "array", AstKindRole::Produces),
    ("plugin-grammar-object", "object", AstKindRole::Produces),
    ("plugin-grammar-match", "match", AstKindRole::Produces),
    ("plugin-grammar-handle", "handle", AstKindRole::Produces),
    ("plugin-grammar-type-annot", "stmt.let", AstKindRole::Produces),
    ("plugin-grammar-stmt-import", "stmt.import", AstKindRole::Produces),
    ("plugin-grammar-stmt-export", "stmt.export", AstKindRole::Produces),
    ("plugin-grammar-stmt-module", "stmt.module", AstKindRole::Produces),
    ("plugin-eval-hello", "hello.greet", AstKindRole::Consumes),
    ("plugin-eval-json", "json", AstKindRole::Consumes),
    ("plugin-typecheck-core", "typecheck", AstKindRole::Consumes),
    ("plugin-fmt-trivia", "fmt.trivia", AstKindRole::Consumes),
    ("plugin-fmt-stmt", "fmt.statements", AstKindRole::Consumes),
    ("plugin-diag-expression", "lint.expression", AstKindRole::Consumes),
];

/// Returns the AST kind declarations for a canonical plugin id: producers for
/// grammar rules and consumers for behavior-level plugins.
fn ast_kinds_for_plugin(id: &str) -> Vec<AstKindDeclaration> {
    AST_KIND_TABLE
        .iter()
        .find(|(pid, _, _)| *pid == id)
        .map(|(_, kind, role)| vec![AstKindDeclaration {
            kind: (*kind).into(),
            role: *role,
        }])
        .unwrap_or_default()
}

/// Synthesizes minimal `PluginManifest`s for the given plugin ids so the
/// resolver has something to select from.
fn manifests_for_plugins(ids: &[String]) -> Vec<PluginManifest> {
    ids.iter()
        .map(|id| PluginManifest {
            id: id.clone(),
            version: "0.1.0".to_string(),
            provides: vec![CapabilityId::from(id.as_str())],
            requires: Vec::new(),
            optional: Vec::new(),
            extends: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
            targets: Vec::new(),
            exclusive: Vec::new(),
            metadata: serde_json::Value::Null,
            ast_kinds: ast_kinds_for_plugin(id),
        })
        .collect()
}

/// Resolves a kit manifest's plugin list into a `ResolvedComposition`,
/// filtering to the grammar-rule plugins the canonical kit understands and
/// returning them in the order the parser expects (primary rules first).
fn resolve_grammar_rules(plugin_ids: &[String]) -> Result<Vec<String>, PlatformError> {
    let manifests = manifests_for_plugins(plugin_ids);
    let composition = handl_platform::resolve(&manifests, &[], None)
        .map_err(|e| PlatformError::Plugin(e.to_string()))?;
    let selected: HashSet<&str> = composition
        .selected_plugins
        .iter()
        .map(|s| s.as_str())
        .collect();
    Ok(GRAMMAR_RULE_PLUGIN_IDS
        .iter()
        .copied()
        .filter(|id| selected.contains(*id))
        .map(|s| s.to_string())
        .collect())
}

/// A curated kit that loads a manifest of plugins and composes a
/// lexer/parser pipeline from them.
#[derive(Debug, Default)]
pub struct Kit {
    selected_grammar_rules: Vec<String>,
}

impl Kit {
    /// Builds the canonical kit from the default manifest.
    pub fn new() -> Self {
        Self::from_manifest(&canonical_manifest()).unwrap_or_default()
    }

    /// Builds a `Kit` from a `KitManifest` by registering the listed plugins.
    pub fn from_manifest(manifest: &KitManifest) -> Result<Self, PlatformError> {
        let selected_grammar_rules = resolve_grammar_rules(&manifest.plugins)?;
        Ok(Self {
            selected_grammar_rules,
        })
    }

    /// Lexes `input` into the kit's token stream.
    pub fn lex(&self, input: &str) -> Result<Vec<Token<DynKind>>, DefaultLexError> {
        DynLexer.lex(input)
    }

    /// Parses `input` into a generic [`KitNode`].
    pub fn parse(&self, input: &str) -> Result<KitNode, PlatformError> {
        let ast = self.parse_to_ast(input)?;
        ast_to_kitnode_with_kit(&ast, self)
    }

    /// Parses `input` into a raw [`AstNode`], bypassing the [`KitNode`]
    /// conversion.
    ///
    /// This is the entry point used by hosts that dispatch evaluation
    /// through the plugin system (ADR 0040): the `Eval` operation's
    /// `Behavior` rules consume [`AstNode`]s directly, decoding their
    /// `payload` fields, so the host needs the raw AST rather than the
    /// higher-level [`KitNode`] representation.
    pub fn parse_to_ast(&self, input: &str) -> Result<AstNode, PlatformError> {
        let tokens = self
            .lex(input)
            .map_err(|e| PlatformError::Plugin(e.to_string()))?;

        let parser = parser_for_plugins(&self.selected_grammar_rules)?;
        parser.parse(&tokens)
    }

    /// Parses a module (a sequence of top-level statements) into a
    /// `Vec<KitNode>`.
    pub fn parse_module(&self, input: &str) -> Result<Vec<KitNode>, PlatformError> {
        let asts = self.parse_module_to_ast(input)?;
        asts.iter().map(|ast| ast_to_kitnode_with_kit(ast, self)).collect()
    }

    /// Parses a module (a sequence of top-level statements) into a
    /// `Vec<AstNode>`, bypassing the [`KitNode`] conversion.
    ///
    /// See [`Kit::parse_to_ast`] for why hosts prefer the raw AST when
    /// dispatching evaluation through the plugin system.
    pub fn parse_module_to_ast(&self, input: &str) -> Result<Vec<AstNode>, PlatformError> {
        let tokens = self
            .lex(input)
            .map_err(|e| PlatformError::Plugin(e.to_string()))?;

        let parser = parser_for_plugins(&self.selected_grammar_rules)?;
        parser.parse_module(&tokens)
    }

    /// Returns the TextMate grammar rules for this kit, collected from
    /// the token factory's [`GrammarContributor`] implementation.
    pub fn grammar_rules(&self) -> Vec<GrammarRule> {
        DynFactory.grammar_rules()
    }

    /// Returns the scope name for this kit's grammar.
    pub fn scope_name(&self) -> &str {
        "source.handl"
    }

    /// Returns the file extensions for this kit's grammar.
    pub fn file_extensions(&self) -> Vec<String> {
        vec!["hnl".to_string()]
    }
}

/// Sub-kit that registers the canonical language operation plugins: the
/// lexer, parser (with all grammar rules), formatter, linter, typechecker,
/// evaluator, builder, compiler, trivia/statement formatters, expression
/// diagnostics, and the options plugin.
pub struct KitLang {
    registered: AtomicBool,
}

impl KitLang {
    /// Create a new `KitLang`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for KitLang {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for KitLang {
    fn id(&self) -> &str {
        "kit.lang"
    }

    fn name(&self) -> &str {
        "kit-lang"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if self.registered.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.registered.store(true, Ordering::SeqCst);

        // Canonical language operation RuleAcceptors
        ctx.register_plugin(BasicLexer::new())?;
        ctx.register_plugin(BasicParser::<DynKind>::new())?;
        let grammar_rules = resolve_grammar_rules(&canonical_manifest().plugins)?;
        for id in &grammar_rules {
            if let Some(rule) = rule_for_plugin_id(id) {
                ctx.contribute::<BasicParser<DynKind>>(rule)?;
            }
        }
        ctx.register_plugin(BasicFormatter::new())?;
        ctx.register_plugin(BasicLinter::new())?;
        ctx.register_plugin(BasicTypechecker::new())?;
        ctx.register_plugin(BasicEvaluator::new())?;
        ctx.register_plugin(BasicBuilder::new())?;
        ctx.register_plugin(BasicCompiler::new())?;

        // Format and lint behavior contributors
        ctx.register_plugin(TriviaFormatter::new())?;
        ctx.register_plugin(StatementFormatter::new())?;
        ctx.register_plugin(ExpressionDiagnostics::new())?;
        ctx.register_plugin(OptionsPlugin::new())?;
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

/// Sub-kit that registers the runtime plugins: the native target, the runtime
/// host, the arithmetic/boolean/array/object/apply builtin families, and the
/// module runtime.
pub struct KitRuntime {
    registered: AtomicBool,
}

impl KitRuntime {
    /// Create a new `KitRuntime`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for KitRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for KitRuntime {
    fn id(&self) -> &str {
        "kit.runtime"
    }

    fn name(&self) -> &str {
        "kit-runtime"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if self.registered.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.registered.store(true, Ordering::SeqCst);

        // Target and library plugins
        ctx.register_plugin(NativeTarget::new())?;

        // Function-registration mechanism.
        //
        // The new `plugin-module` `ModuleRuntime` is the single function-
        // registration mechanism (ADR 0040). All `plugin-lib-*` crates now
        // contribute into it via the new non-generic `Module` trait.
        ctx.register_plugin(ModuleRuntime::new())?;
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

/// Sub-kit that registers the out-of-process plugin loaders. The
/// `LoaderHostPlugin` is registered first so the loaders' `contribute` methods
/// can find it.
pub struct KitLoaders {
    registered: AtomicBool,
}

impl KitLoaders {
    /// Create a new `KitLoaders`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for KitLoaders {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for KitLoaders {
    fn id(&self) -> &str {
        "kit.loaders"
    }

    fn name(&self) -> &str {
        "kit-loaders"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if self.registered.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.registered.store(true, Ordering::SeqCst);

        // Out-of-process plugin loaders. `LoaderHostPlugin` must be registered
        // before the loaders so their `contribute` methods can find it.
        ctx.register_plugin(LoaderHostPlugin::default())?;
        ctx.register_plugin(NativeLoader::new())?;
        ctx.register_plugin(WasmLoader::new())?;
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

/// Sub-kit that registers the eval plugins: the hello and json eval plugins.
pub struct KitEval {
    registered: AtomicBool,
}

impl KitEval {
    /// Create a new `KitEval`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for KitEval {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for KitEval {
    fn id(&self) -> &str {
        "kit.eval"
    }

    fn name(&self) -> &str {
        "kit-eval"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if self.registered.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.registered.store(true, Ordering::SeqCst);

        // Existing reference eval plugins (pre-ADR-0040).
        ctx.register_plugin(HelloEvalPlugin::new())?;
        ctx.register_plugin(JsonEvalPlugin::new())?;

        // New granular eval plugins (ADR 0040). These are `EvalBehavior` rules
        // contributed into `BasicEvaluator`, mirroring the `plugin-grammar-*`
        // pattern. They coexist with the legacy VM-based eval (which is
        // dispatched through a separate mechanism) until the VM layer is
        // deleted in step 12.
        ctx.register_plugin(EvalCorePlugin::new())?;
        ctx.register_plugin(EvalValuePlugin::new())?;
        ctx.register_plugin(EvalCallPlugin::new())?;
        ctx.register_plugin(EvalArrayPlugin::new())?;
        ctx.register_plugin(EvalObjectPlugin::new())?;
        ctx.register_plugin(EvalIndexPlugin::new())?;
        ctx.register_plugin(EvalFieldAccessPlugin::new())?;
        ctx.register_plugin(EvalHandlePlugin::new())?;
        ctx.register_plugin(EvalAssignPlugin::new())?;
        ctx.register_plugin(EvalFunctionPlugin::new())?;

        // Basic exec-control plugins for safety. A sandbox kit would add more
        // (deny, allow, etc.).
        ctx.register_plugin(ExecMaxDepthPlugin::new(1000))?;
        ctx.register_plugin(ExecMaxCallsPlugin::new(10000))?;
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

/// `Library` plugin for the canonical kit.
///
/// `CanonicalKit` composes the four sub-kits — [`KitLang`], [`KitRuntime`],
/// [`KitLoaders`], and [`KitEval`] — so that downstream kits can pick and
/// choose individual groups of plugins instead of pulling in everything.
pub struct CanonicalKit {
    registered: AtomicBool,
}

impl CanonicalKit {
    /// Create a new `CanonicalKit`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for CanonicalKit {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for CanonicalKit {
    fn id(&self) -> &str {
        "kit-canonical"
    }

    fn name(&self) -> &str {
        "Canonical kit"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if self.registered.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.registered.store(true, Ordering::SeqCst);

        // The canonical manifest lists token, AST-shape, and grammar-rule plugins
        // (plugin-token-*, plugin-ast-*, plugin-grammar-*). These crates provide
        // shared types and rules rather than `Plugin` implementations, so they are
        // wired directly by the existing `Kit` lexer/parser pipeline and are not
        // registered here.

        ctx.register_plugin(KitLang::new())?;
        ctx.register_plugin(KitRuntime::new())?;
        ctx.register_plugin(KitLoaders::new())?;
        ctx.register_plugin(KitEval::new())?;
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_lexer_template::{TemplateSegment, TemplateTokenKind};
    use handl_plugin_grammar_template::TemplateToken;
    use handl_plugin_token_kind::TemplateKind;
    use handl_plugin_token_number::NumberKind;
    use handl_plugin_token_string::StringKind;

    #[test]
    fn lex_number() {
        let kit = Kit::new();
        let tokens = kit.lex("42").unwrap();
        assert_eq!(tokens.len(), 2); // number + eof
        assert_eq!(tokens[0].kind, DynKind(Box::new(NumberKind::Number(42.0))));
    }

    #[test]
    fn lex_string() {
        let kit = Kit::new();
        let tokens = kit.lex("\"hi\"").unwrap();
        assert_eq!(tokens[0].kind, DynKind(Box::new(StringKind::String("hi".into()))));
    }

    #[test]
    fn parse_number() {
        let kit = Kit::new();
        assert_eq!(
            kit.parse("1").unwrap(),
            KitNode::Literal(KitValue::Number(1.0))
        );
    }

    #[test]
    fn parse_call() {
        let kit = Kit::new();
        let node = kit.parse("add(1, 2)").unwrap();
        let expected = KitNode::Call(Call::new(
            "add",
            vec![
                KitNode::Literal(KitValue::Number(1.0)),
                KitNode::Literal(KitValue::Number(2.0)),
            ],
        ));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_index() {
        let kit = Kit::new();
        let node = kit.parse("arr[0]").unwrap();
        let expected = KitNode::Index(Box::new(Index::new(
            KitNode::Var(Var::new("arr".into())),
            KitNode::Literal(KitValue::Number(0.0)),
        )));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_pipe() {
        let kit = Kit::new();
        let node = kit.parse("1 |> add(2, 3)").unwrap();
        let right = KitNode::Call(Call::new(
            "add",
            vec![
                KitNode::Literal(KitValue::Number(2.0)),
                KitNode::Literal(KitValue::Number(3.0)),
            ],
        ));
        let expected = KitNode::Pipe(Box::new(Pipe::new(
            KitNode::Literal(KitValue::Number(1.0)),
            right,
        )));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_let() {
        let kit = Kit::new();
        let node = kit.parse("a = 1").unwrap();
        let expected = KitNode::Assign(Box::new(Assign::new(
            KitNode::Var(Var::new("a".into())),
            KitNode::Literal(KitValue::Number(1.0)),
        )));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_string_literal() {
        let kit = Kit::new();
        let node = kit.parse("\"hi\"").unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::String("hi".into())));
    }

    #[test]
    fn parse_bool_literal() {
        let kit = Kit::new();
        assert_eq!(
            kit.parse("true").unwrap(),
            KitNode::Literal(KitValue::Bool(true))
        );
        assert_eq!(
            kit.parse("false").unwrap(),
            KitNode::Literal(KitValue::Bool(false))
        );
    }

    #[test]
    fn parse_null_literal() {
        let kit = Kit::new();
        assert_eq!(kit.parse("null").unwrap(), KitNode::Literal(KitValue::Null));
    }

    #[test]
    fn parse_module_empty() {
        let kit = Kit::new();
        let stmts = kit.parse_module("").unwrap();
        assert!(stmts.is_empty());
    }

    #[test]
    fn parse_module_single_let() {
        let kit = Kit::new();
        let stmts = kit.parse_module("a = 1;").unwrap();
        assert_eq!(stmts.len(), 1);
        let expected = KitNode::Assign(Box::new(Assign::new(
            KitNode::Var(Var::new("a".into())),
            KitNode::Literal(KitValue::Number(1.0)),
        )));
        assert_eq!(stmts[0], expected);
    }

    #[test]
    fn parse_module_multiple_let() {
        let kit = Kit::new();
        let stmts = kit.parse_module("a = 1; b = 2;").unwrap();
        assert_eq!(stmts.len(), 2);
        assert_eq!(
            stmts[0],
            KitNode::Assign(Box::new(Assign::new(
                KitNode::Var(Var::new("a".into())),
                KitNode::Literal(KitValue::Number(1.0)),
            )))
        );
        assert_eq!(
            stmts[1],
            KitNode::Assign(Box::new(Assign::new(
                KitNode::Var(Var::new("b".into())),
                KitNode::Literal(KitValue::Number(2.0)),
            )))
        );
    }

    #[test]
    fn parse_module_const() {
        let kit = Kit::new();
        let stmts = kit.parse_module("x: number = 42;").unwrap();
        assert_eq!(stmts.len(), 1);
        let expected = KitNode::Let(Box::new(Let::new(
            "x".into(),
            KitNode::Literal(KitValue::Number(42.0)),
        )));
        assert_eq!(stmts[0], expected);
    }

    #[test]
    fn parse_module_expression_statement() {
        let kit = Kit::new();
        let stmts = kit.parse_module("42").unwrap();
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], KitNode::Literal(KitValue::Number(42.0)));
    }

    #[test]
    fn parse_module_mixed() {
        let kit = Kit::new();
        let stmts = kit.parse_module("a = 1; 42").unwrap();
        assert_eq!(stmts.len(), 2);
        assert_eq!(
            stmts[0],
            KitNode::Assign(Box::new(Assign::new(
                KitNode::Var(Var::new("a".into())),
                KitNode::Literal(KitValue::Number(1.0)),
            )))
        );
        assert_eq!(stmts[1], KitNode::Literal(KitValue::Number(42.0)));
    }

    #[test]
    fn parse_module_import() {
        let kit = Kit::new();
        let stmts = kit.parse_module("import \"./lib\"").unwrap();
        assert_eq!(stmts.len(), 1);
        let expected = KitNode::Import(Box::new(Import::new(KitNode::Literal(KitValue::String(
            "./lib".into(),
        )))));
        assert_eq!(stmts[0], expected);
    }

    #[test]
    fn parse_module_export() {
        let kit = Kit::new();
        let stmts = kit.parse_module("export 42").unwrap();
        assert_eq!(stmts.len(), 1);
        let expected = KitNode::Export(Box::new(Export::new(KitNode::Literal(KitValue::Number(
            42.0,
        )))));
        assert_eq!(stmts[0], expected);
    }

    #[test]
    fn parse_module_module_decl() {
        let kit = Kit::new();
        let stmts = kit.parse_module("module mymod").unwrap();
        assert_eq!(stmts.len(), 1);
        let expected = KitNode::Module(Box::new(Module::new("mymod".into(), None)));
        assert_eq!(stmts[0], expected);
    }

    #[test]
    fn parse_module_module_with_value() {
        let kit = Kit::new();
        let stmts = kit.parse_module("module mymod = 42").unwrap();
        assert_eq!(stmts.len(), 1);
        let expected = KitNode::Module(Box::new(Module::new(
            "mymod".into(),
            Some(KitNode::Literal(KitValue::Number(42.0))),
        )));
        assert_eq!(stmts[0], expected);
    }

    #[test]
    fn parse_apply() {
        let kit = Kit::new();
        let node = kit.parse("(1)").unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::Number(1.0)));
    }

    #[test]
    fn parse_assign() {
        let kit = Kit::new();
        let node = kit.parse("a = 1").unwrap();
        let expected = KitNode::Assign(Box::new(Assign::new(
            KitNode::Var(Var::new("a".into())),
            KitNode::Literal(KitValue::Number(1.0)),
        )));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_array() {
        let kit = Kit::new();
        let node = kit.parse("[1, 2]").unwrap();
        let expected = KitNode::Array(Array::new(vec![
            KitNode::Literal(KitValue::Number(1.0)),
            KitNode::Literal(KitValue::Number(2.0)),
        ]));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_object() {
        let kit = Kit::new();
        let node = kit.parse("{a: 1}").unwrap();
        let expected = KitNode::Object(Object::new(vec![(
            "a".into(),
            KitNode::Literal(KitValue::Number(1.0)),
        )]));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_match() {
        let kit = Kit::new();
        let node = kit.parse("match x").unwrap();
        let expected = KitNode::Match(Box::new(Match::new(KitNode::Var(Var::new("x".into())))));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_match_with_literal_arms() {
        let kit = Kit::new();
        let node = kit.parse(r#"match 1 { 1: "one", _: "other" }"#).unwrap();
        let expected = KitNode::Match(Box::new(Match::with_arms(
            KitNode::Literal(KitValue::Number(1.0)),
            vec![
                MatchArm {
                    pattern: KitNode::Literal(KitValue::Number(1.0)),
                    guard: None,
                    body: KitNode::Literal(KitValue::String("one".into())),
                    metadata: (),
                },
                MatchArm {
                    pattern: KitNode::Var(Var::new("_".into())),
                    guard: None,
                    body: KitNode::Literal(KitValue::String("other".into())),
                    metadata: (),
                },
            ],
        )));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_match_with_guard() {
        let kit = Kit::new();
        let node = kit
            .parse(r#"match 5 { x: cmp.gt(x, 3) : "big", _: "small" }"#)
            .unwrap();
        let expected = KitNode::Match(Box::new(Match::with_arms(
            KitNode::Literal(KitValue::Number(5.0)),
            vec![
                MatchArm {
                    pattern: KitNode::Var(Var::new("x".into())),
                    guard: Some(KitNode::Apply(Box::new(Apply::new(
                        KitNode::Index(Box::new(Index::new(
                            KitNode::Var(Var::new("cmp".into())),
                            KitNode::Literal(KitValue::String("gt".into())),
                        ))),
                        vec![
                            KitNode::Var(Var::new("x".into())),
                            KitNode::Literal(KitValue::Number(3.0)),
                        ],
                    )))),
                    body: KitNode::Literal(KitValue::String("big".into())),
                    metadata: (),
                },
                MatchArm {
                    pattern: KitNode::Var(Var::new("_".into())),
                    guard: None,
                    body: KitNode::Literal(KitValue::String("small".into())),
                    metadata: (),
                },
            ],
        )));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_match_trailing_comma() {
        let kit = Kit::new();
        let node = kit.parse(r#"match 1 { 1: "one", }"#).unwrap();
        let expected = KitNode::Match(Box::new(Match::with_arms(
            KitNode::Literal(KitValue::Number(1.0)),
            vec![MatchArm {
                pattern: KitNode::Literal(KitValue::Number(1.0)),
                guard: None,
                body: KitNode::Literal(KitValue::String("one".into())),
                metadata: (),
            }],
        )));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_handle() {
        let kit = Kit::new();
        let node = kit.parse("handle x").unwrap();
        let expected = KitNode::Handle(Box::new(Handle::new(KitNode::Var(Var::new("x".into())))));
        assert_eq!(node, expected);
    }

    #[test]
    fn from_manifest_subset_parses_values_but_not_calls() {
        let manifest = KitManifest {
            id: "subset".to_string(),
            version: "0.1.0".to_string(),
            plugins: vec![
                "plugin-grammar-primary-value".to_string(),
                "plugin-grammar-type-annot".to_string(),
                "plugin-grammar-stmt-import".to_string(),
            ],
        };
        let kit = Kit::from_manifest(&manifest).unwrap();
        assert_eq!(
            kit.parse("42").unwrap(),
            KitNode::Literal(KitValue::Number(42.0))
        );
        assert!(kit.parse("[1, 2]").is_err());
    }

    #[test]
    fn canonical_manifest_passes_ast_kind_validation() {
        let manifest = canonical_manifest();
        let manifests = manifests_for_plugins(&manifest.plugins);
        let selected: HashSet<String> = manifests.iter().map(|m| m.id.clone()).collect();
        assert!(handl_platform::resolve::validate_ast_kinds(&manifests, &selected).is_ok());
    }

    #[test]
    fn subset_missing_producer_fails_ast_kind_validation() {
        let grammar_call = manifests_for_plugins(&["plugin-grammar-call".to_string()])[0].clone();
        let grammar_value =
            manifests_for_plugins(&["plugin-grammar-primary-value".to_string()])[0].clone();
        let mut consumer = grammar_call.clone();
        consumer.id = "consumer-of-call".to_string();
        consumer.ast_kinds = vec![AstKindDeclaration {
            kind: "call".into(),
            role: AstKindRole::Consumes,
        }];
        let all = vec![grammar_call, grammar_value, consumer];
        let selected: HashSet<String> = ["plugin-grammar-primary-value", "consumer-of-call"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let err = handl_platform::resolve::validate_ast_kinds(&all, &selected).unwrap_err();
        assert!(matches!(
            err,
            handl_platform::resolve::ResolutionError::MissingProducer { ref kind, ref consumed_by, .. }
            if kind == "call" && consumed_by == "consumer-of-call"
        ));
    }

    // --- Template string integration tests ---

    #[test]
    fn lex_template_simple() {
        let kit = Kit::new();
        let tokens = kit.lex("`hello`").unwrap();
        assert_eq!(tokens.len(), 2); // template + eof
        assert_eq!(
            tokens[0].kind,
            DynKind(Box::new(TemplateKind(vec![TemplateSegment::Text("hello".into())])))
        );
    }

    #[test]
    fn lex_template_with_interpolation() {
        let kit = Kit::new();
        let tokens = kit.lex("`hi ${name}!`").unwrap();
        assert_eq!(
            tokens[0].kind,
            DynKind(Box::new(TemplateKind(vec![
                TemplateSegment::Text("hi ".into()),
                TemplateSegment::Expr("name".into()),
                TemplateSegment::Text("!".into()),
            ])))
        );
    }

    #[test]
    fn lex_template_empty() {
        let kit = Kit::new();
        let tokens = kit.lex("``").unwrap();
        assert_eq!(tokens[0].kind, DynKind(Box::new(TemplateKind(vec![]))));
    }

    #[test]
    fn lex_template_multiple_interpolations() {
        let kit = Kit::new();
        let tokens = kit.lex("`${a} and ${b}`").unwrap();
        assert_eq!(
            tokens[0].kind,
            DynKind(Box::new(TemplateKind(vec![
                TemplateSegment::Expr("a".into()),
                TemplateSegment::Text(" and ".into()),
                TemplateSegment::Expr("b".into()),
            ])))
        );
    }

    #[test]
    fn lex_template_unterminated_errors() {
        let kit = Kit::new();
        assert!(kit.lex("`oops").is_err());
    }

    #[test]
    fn lex_template_unterminated_interpolation_errors() {
        let kit = Kit::new();
        assert!(kit.lex("`hi ${name").is_err());
    }

    #[test]
    fn parse_template_text_only() {
        let kit = Kit::new();
        let node = kit.parse("`hello`").unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::String("hello".into())));
    }

    #[test]
    fn parse_template_empty() {
        let kit = Kit::new();
        let node = kit.parse("``").unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::String(String::new())));
    }

    #[test]
    fn parse_template_expr_only() {
        let kit = Kit::new();
        let node = kit.parse("`${42}`").unwrap();
        // Only one part — returns it directly (no concat needed)
        assert_eq!(node, KitNode::Literal(KitValue::Number(42.0)));
    }

    #[test]
    fn parse_template_text_and_expr() {
        let kit = Kit::new();
        let node = kit.parse("`hello ${42}!`").unwrap();
        // str.concat("hello ", str.concat(42, "!"))
        let expected = KitNode::Call(Call::new("str.concat", vec![
            KitNode::Literal(KitValue::String("hello ".into())),
            KitNode::Call(Call::new("str.concat", vec![
                KitNode::Literal(KitValue::Number(42.0)),
                KitNode::Literal(KitValue::String("!".into())),
            ])),
        ]));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_template_multiple_interpolations() {
        let kit = Kit::new();
        let node = kit.parse("`${1}${2}`").unwrap();
        // str.concat(1, 2)
        let expected = KitNode::Call(Call::new("str.concat", vec![
            KitNode::Literal(KitValue::Number(1.0)),
            KitNode::Literal(KitValue::Number(2.0)),
        ]));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_template_complex() {
        let kit = Kit::new();
        let node = kit.parse("`a${1}b${2}c`").unwrap();
        // str.concat("a", str.concat(1, str.concat("b", str.concat(2, "c"))))
        let expected = KitNode::Call(Call::new("str.concat", vec![
            KitNode::Literal(KitValue::String("a".into())),
            KitNode::Call(Call::new("str.concat", vec![
                KitNode::Literal(KitValue::Number(1.0)),
                KitNode::Call(Call::new("str.concat", vec![
                    KitNode::Literal(KitValue::String("b".into())),
                    KitNode::Call(Call::new("str.concat", vec![
                        KitNode::Literal(KitValue::Number(2.0)),
                        KitNode::Literal(KitValue::String("c".into())),
                    ])),
                ])),
            ])),
        ]));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_template_with_string_expr() {
        let kit = Kit::new();
        let node = kit.parse("`${\"inner\"}`").unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::String("inner".into())));
    }

    #[test]
    fn parse_template_with_var_expr() {
        let kit = Kit::new();
        let node = kit.parse("`${name}`").unwrap();
        assert_eq!(node, KitNode::Var(Var::new("name".into())));
    }

    #[test]
    fn parse_template_multiline() {
        let kit = Kit::new();
        let node = kit.parse("`line1\nline2`").unwrap();
        assert_eq!(
            node,
            KitNode::Literal(KitValue::String("line1\nline2".into()))
        );
    }

    #[test]
    fn canonical_kind_template_display() {
        let kind = DynKind(Box::new(TemplateKind(vec![
            TemplateSegment::Text("hello ".into()),
            TemplateSegment::Expr("name".into()),
        ])));
        let s = format!("{kind}");
        assert!(s.starts_with("template("));
        assert!(s.contains("\"hello \""));
        assert!(s.contains("${name}"));
        assert!(s.ends_with(")"));
    }

    #[test]
    fn canonical_kind_template_display_text_only() {
        let kind = DynKind(Box::new(TemplateKind(vec![TemplateSegment::Text("hi".into())])));
        let s = format!("{kind}");
        assert_eq!(s, "template(\"hi\")");
    }

    #[test]
    fn canonical_kind_template_display_empty() {
        let kind = DynKind(Box::new(TemplateKind(vec![])));
        let s = format!("{kind}");
        assert_eq!(s, "template()");
    }

    #[test]
    fn template_token_kind_constructs() {
        let kind = DynKind::template_kind(vec![TemplateSegment::Text("test".into())]);
        assert!(kind.downcast_ref::<TemplateKind>().is_some());
    }

    #[test]
    fn template_token_returns_segments() {
        let kind = DynKind(Box::new(TemplateKind(vec![
            TemplateSegment::Text("a".into()),
            TemplateSegment::Expr("b".into()),
        ])));
        let segs = kind.as_template_segments().unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], TemplateSegment::Text("a".into()));
        assert_eq!(segs[1], TemplateSegment::Expr("b".into()));
    }

    #[test]
    fn template_token_returns_none_for_non_template() {
        assert!(DynKind(Box::new(NumberKind::Number(1.0))).as_template_segments().is_none());
        assert!(DynKind(Box::new(StringKind::String("x".into()))).as_template_segments().is_none());
        use handl_plugin_lexer::EofKind;
        assert!(DynKind(Box::new(EofKind)).as_template_segments().is_none());
    }

    #[test]
    fn ast_to_kitnode_with_kit_missing_segments_errors() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({}),
        };
        let kit = Kit::new();
        let result = ast_to_kitnode_with_kit(&ast, &kit);
        assert!(result.is_err());
    }

    #[test]
    fn ast_to_kitnode_with_kit_missing_type_errors() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({
                "segments": [serde_json::json!({ "value": "hi" })]
            }),
        };
        let kit = Kit::new();
        let result = ast_to_kitnode_with_kit(&ast, &kit);
        assert!(result.is_err());
    }

    #[test]
    fn ast_to_kitnode_with_kit_unknown_segment_type_errors() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({
                "segments": [serde_json::json!({ "type": "bogus" })]
            }),
        };
        let kit = Kit::new();
        let result = ast_to_kitnode_with_kit(&ast, &kit);
        assert!(result.is_err());
    }

    #[test]
    fn ast_to_kitnode_with_kit_missing_value_in_text_errors() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({
                "segments": [serde_json::json!({ "type": "text" })]
            }),
        };
        let kit = Kit::new();
        let result = ast_to_kitnode_with_kit(&ast, &kit);
        assert!(result.is_err());
    }

    #[test]
    fn ast_to_kitnode_with_kit_missing_raw_in_expr_errors() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({
                "segments": [serde_json::json!({ "type": "expr" })]
            }),
        };
        let kit = Kit::new();
        let result = ast_to_kitnode_with_kit(&ast, &kit);
        assert!(result.is_err());
    }

    #[test]
    fn ast_to_kitnode_with_kit_empty_segments() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({ "segments": [] }),
        };
        let kit = Kit::new();
        let node = ast_to_kitnode_with_kit(&ast, &kit).unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::String(String::new())));
    }

    #[test]
    fn ast_to_kitnode_with_kit_single_text() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({
                "segments": [serde_json::json!({ "type": "text", "value": "hello" })]
            }),
        };
        let kit = Kit::new();
        let node = ast_to_kitnode_with_kit(&ast, &kit).unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::String("hello".into())));
    }

    #[test]
    fn ast_to_kitnode_with_kit_single_expr() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({
                "segments": [serde_json::json!({ "type": "expr", "raw": "42" })]
            }),
        };
        let kit = Kit::new();
        let node = ast_to_kitnode_with_kit(&ast, &kit).unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::Number(42.0)));
    }

    #[test]
    fn ast_to_kitnode_with_kit_delegates_non_template() {
        let ast = AstNode {
            kind: "value".into(),
            span: None,
            payload: serde_json::json!({ "value": 42 }),
        };
        let kit = Kit::new();
        let node = ast_to_kitnode_with_kit(&ast, &kit).unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::Number(42.0)));
    }

    #[test]
    fn canonical_kit_registers_loaders() {
        use handl_platform::{DefaultHost, CallableLookup};

        let mut host = DefaultHost::new();
        host.register_plugin(CanonicalKit::new());
        host.load().unwrap();

        assert!(host.plugins().has_id("loader.host"));
        assert!(host.plugins().has_id("loader.native"));
        assert!(host.plugins().has_id("loader.wasm"));
        assert!(host.plugins().resolve(&"load".into()).is_some());
    }
}
