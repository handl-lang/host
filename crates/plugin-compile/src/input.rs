use handl_language::{Ast, AstRef, Language};
use hnl_engine::Term;
use hnl_engine_typecheck::TypecheckReport;

/// The input to a [`Compiler`](crate::Compiler).
#[derive(Debug, Clone)]
pub struct CompilerInput {
    /// The source text that produced the AST.
    pub source: String,
    /// The full AST arena.
    pub ast: Ast,
    /// The root of the compilation unit inside `ast`.
    pub root: AstRef,
    /// The language definition in scope.
    pub language: Language,
    /// The lowered root `Term` for the compilation unit.
    pub term: Term,
    /// The typecheck report for the input.
    pub report: TypecheckReport,
}

impl CompilerInput {
    /// Construct a new compiler input.
    pub fn new(
        source: impl Into<String>,
        ast: Ast,
        root: AstRef,
        language: Language,
        term: Term,
        report: TypecheckReport,
    ) -> Self {
        Self {
            source: source.into(),
            ast,
            root,
            language,
            term,
            report,
        }
    }
}
