use handl_platform::language::{LexerContext, LexerPlugin, Span, Token};

pub struct Comments;

impl LexerPlugin for Comments {
    fn lex(&self, input: &str, ctx: &mut LexerContext) {
        let mut i = 0;
        let bytes = input.as_bytes();
        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
                let start = i;
                i += 2;
                let text_start = i;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                let text = input[text_start..i].to_string();
                let end = i;
                ctx.emit(Token {
                    kind: "line_comment".into(),
                    span: Span { start, end },
                    text,
                });
                if i < bytes.len() {
                    i += 1;
                }
            } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                let start = i;
                i += 2;
                let text_start = i;
                let mut found = false;
                while i + 1 < bytes.len() {
                    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        found = true;
                        break;
                    }
                    i += 1;
                }
                if found {
                    let text = input[text_start..i].to_string();
                    i += 2;
                    let end = i;
                    ctx.emit(Token {
                        kind: "block_comment".into(),
                        span: Span { start, end },
                        text,
                    });
                } else {
                    ctx.error(handl_platform::language::Diagnostic {
                        level: handl_platform::language::DiagnosticLevel::Error,
                        message: "unterminated block comment".to_string(),
                        span: Some(Span {
                            start,
                            end: bytes.len(),
                        }),
                    });
                    i = bytes.len();
                }
            } else {
                i += 1;
            }
        }
    }
}
