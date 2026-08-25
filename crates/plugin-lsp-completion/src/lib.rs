use handl_ast_canonical::Expr;
use handl_expr::{ModuleOutput, StmtKind};
use handl_kit_canonical::CanonicalKit;
use handl_platform::PlatformError;
use handl_tooling::lsp::{
    CompletionItem, CompletionPlugin, CompletionRequest, CompletionResponse,
};

/// A simple completion plugin that offers in-scope `let`/`const` bindings and
/// a small set of keywords.
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicCompletion;

impl BasicCompletion {
    pub fn new() -> Self {
        Self
    }
}

const KEYWORDS: &[&str] = &[
    "let", "const", "import", "export", "module", "handle", "match", "not", "when", "else",
    "is", "must", "may", "typeof", "as",
];

impl CompletionPlugin for BasicCompletion {
    fn run(&self, request: &CompletionRequest) -> Result<CompletionResponse, PlatformError> {
        let kit = CanonicalKit::new();
        let ModuleOutput { stmts, .. } = kit.parse_module_recovered(&request.source);

        let mut names = Vec::new();
        for stmt in &stmts {
            match &stmt.kind {
                StmtKind::Let { pattern, .. } | StmtKind::Const { pattern, .. } => {
                    for n in pattern.bound_names() {
                        if !names.contains(&n) {
                            names.push(n);
                        }
                    }
                }
                _ => {}
            }
        }

        for stmt in &stmts {
            if let StmtKind::Expr(expr) = &stmt.kind {
                if let Some(id) = first_identifier(expr) {
                    if !names.contains(&id) {
                        names.push(id);
                    }
                }
            }
        }

        let prefix = identifier_prefix(&request.source, request.pos);

        let mut items = Vec::new();
        for name in names {
            if name.starts_with(&prefix) && !items.iter().any(|i: &CompletionItem| i.label == name) {
                items.push(CompletionItem::new(name));
            }
        }
        for kw in KEYWORDS {
            if kw.starts_with(&prefix) && !items.iter().any(|i: &CompletionItem| i.label == *kw) {
                items.push(CompletionItem::new(*kw));
            }
        }

        Ok(CompletionResponse(items))
    }
}

fn identifier_prefix(source: &str, pos: usize) -> String {
    let before = &source[..pos.min(source.len())];
    let mut rev = String::new();
    for ch in before.chars().rev() {
        if ch.is_alphanumeric() || ch == '_' {
            rev.push(ch);
        } else {
            break;
        }
    }
    rev.chars().rev().collect()
}

fn first_identifier<V>(expr: &Expr<V>) -> Option<String> {
    match expr {
        Expr::Var(name) => Some(name.clone()),
        _ => None,
    }
}
