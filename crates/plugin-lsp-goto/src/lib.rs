use handl_expr::Token;
use handl_kit_canonical::CanonicalKit;
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;
use handl_tooling::lsp::{GotoDefPlugin, GotoDefRequest, GotoDefResponse, Location};

/// A minimal goto-definition plugin that jumps from an identifier use to the
/// nearest preceding `let`/`const` binding with the same name.
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicGotoDef;

impl BasicGotoDef {
    pub fn new() -> Self {
        Self
    }
}

impl GotoDefPlugin for BasicGotoDef {
    fn run(&self, request: &GotoDefRequest) -> Result<GotoDefResponse, PlatformError> {
        let kit = CanonicalKit::new();
        let tokens = kit.lex(&request.source);

        let target = identifier_at_or_before(&tokens, request.pos);

        if target.is_empty() {
            return Ok(GotoDefResponse(None));
        }

        for i in 0..tokens.len().saturating_sub(1) {
            let name = match &tokens[i].kind {
                CanonicalKind::Identifier(name) => name.as_str(),
                _ => continue,
            };
            if !is_binding_keyword(name) {
                continue;
            }
            if let CanonicalKind::Identifier(id) = &tokens[i + 1].kind {
                if id == &target {
                    return Ok(GotoDefResponse(Some(Location::new(
                        tokens[i + 1].span.start,
                        tokens[i + 1].span.end,
                    ))));
                }
            }
        }

        Ok(GotoDefResponse(None))
    }
}

fn is_binding_keyword(name: &str) -> bool {
    name == "let" || name == "const"
}

fn identifier_at_or_before(tokens: &[Token], pos: usize) -> String {
    if let Some(t) = tokens.iter().find(|t| {
        matches!(&t.kind, CanonicalKind::Identifier(_)) && t.span.start <= pos && pos <= t.span.end
    }) {
        if let CanonicalKind::Identifier(name) = &t.kind {
            return name.clone();
        }
    }

    let mut best = String::new();
    let mut best_end = 0usize;
    for t in tokens.iter() {
        if let CanonicalKind::Identifier(name) = &t.kind {
            if t.span.end <= pos && t.span.end > best_end {
                best = name.clone();
                best_end = t.span.end;
            }
        }
    }
    best
}
