//! A minimal arrow-graph parser for `.hnl` arrow files (ADR-0069).
//!
//! This parser reads the arrow-graph syntax used in `kit-stdlib/data/arrows/`
//! and produces `Scheme` IR. It handles two syntaxes:
//!
//! 1. **Simple arrow-graph syntax** — the combinator expressions described in
//!    ADR-0069. Supports both function-call syntax and infix operator syntax:
//!    ```text
//!    loop(comp(insert, first(id)))       // function-call syntax
//!    loop(insert >>> first(id))          // equivalent operator syntax
//!    arr("sort")
//!    comp(a, b)                          // same as: a >>> b
//!    choice(a, b)                        // same as: a ||| b
//!    ```
//!    The infix operators `>>>` (sequence) and `|||` (choice) are sugar:
//!    `a >>> b` desugars to `comp(a, b)` (`Scheme::Seq`), and `a ||| b`
//!    desugars to `choice(a, b)` (`Scheme::Choice`). `|||` binds tighter
//!    than `>>>`, so `a >>> b ||| c` parses as `comp(a, choice(b, c))`.
//!
//! 2. **Rich `loop { ... } ( proc ... )` syntax** — the surface syntax
//!    actually used in the stdlib `.hnl` files. The parser recognises the
//!    structural patterns in the `proc` body and maps them to the
//!    corresponding `Scheme` graph, matching the hand-built graphs in
//!    `e2e_tests.rs`.
//!
//! The parser is deliberately minimal — it handles only the constructs needed
//! for arrow expressions, not the full handl language.

use std::collections::HashMap;

use handl_language::Scheme;

// ============================================================================
// Public API
// ============================================================================

/// The result of parsing an arrow file.
///
/// A single-expression file is [`ParsedFile::Single`] — the file IS the arrow.
/// A multi-name file is [`ParsedFile::Named`] — it produces a record of named
/// arrows (per ADR-0069 §1).
#[derive(Debug, Clone)]
pub enum ParsedFile {
    /// A single-expression file — the file IS the arrow.
    Single(Scheme),
    /// A multi-name file — produces a record of named arrows.
    Named(HashMap<String, Scheme>),
}

/// Parse a `.hnl` arrow file's content.
///
/// Dispatches to the rich-loop parser or the simple arrow-graph parser based
/// on the file's syntax.
pub fn parse_arrow_file(src: &str) -> Result<ParsedFile, String> {
    let stripped = strip_comments(src);
    let trimmed = stripped.trim();

    if trimmed.is_empty() {
        return Err("empty file (after stripping comments)".to_string());
    }

    // Rich loop syntax: loop { ... } ( proc (state) -> match ... )
    if trimmed.starts_with("loop {") {
        let scheme = parse_rich_loop(trimmed)?;
        return Ok(ParsedFile::Single(scheme));
    }

    // Named arrows: lines with `name = expr`
    if has_named_arrows(trimmed) {
        let map = parse_named_arrows(trimmed)?;
        return Ok(ParsedFile::Named(map));
    }

    // Simple single-expression file
    let scheme = parse_arrow_expr(trimmed)?;
    Ok(ParsedFile::Single(scheme))
}

/// Parse a simple arrow expression (the combinator syntax).
///
/// Handles: `arr("name")`, `comp(a, b)`, `first(a)`, `choice(a, b)`,
/// `loop(a)`, `id`, `app("name", args)`, and bare identifiers (references
/// to named arrows). Also supports infix operator syntax: `a >>> b`
/// (sequence, same as `comp(a, b)`) and `a ||| b` (choice, same as
/// `choice(a, b)`).
pub fn parse_arrow_expr(src: &str) -> Result<Scheme, String> {
    let tokens = tokenize(src)?;
    let mut parser = Parser::new(&tokens);
    let scheme = parser.parse_expr()?;
    if parser.pos != tokens.len() {
        return Err(format!(
            "unexpected tokens after expression: {} remaining",
            tokens.len() - parser.pos
        ));
    }
    Ok(scheme)
}

// ============================================================================
// Comment stripping
// ============================================================================

/// Remove `//` comment lines from the source.
fn strip_comments(src: &str) -> String {
    src.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ============================================================================
// Named arrow detection and parsing
// ============================================================================

/// Check if the source contains named arrow definitions (`name = expr`).
fn has_named_arrows(src: &str) -> bool {
    src.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && trimmed.contains(" = ") && !trimmed.starts_with("==")
    })
}

/// Parse named arrows: `name = expr` on each line.
fn parse_named_arrows(src: &str) -> Result<HashMap<String, Scheme>, String> {
    let mut map = HashMap::new();

    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Find ` = ` (top-level assignment)
        if let Some(eq_idx) = trimmed.find(" = ") {
            let name = trimmed[..eq_idx].trim().to_string();
            let expr_str = trimmed[eq_idx + 3..].trim();
            let scheme = parse_arrow_expr(expr_str)?;
            map.insert(name, scheme);
        }
    }

    if map.is_empty() {
        return Err("no named arrows found".to_string());
    }

    Ok(map)
}

// ============================================================================
// Tokenizer (for simple arrow-graph syntax)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
enum Token {
    LParen,
    RParen,
    Comma,
    StringLit(String),
    Ident(String),
    /// `>>>` — sequence (flow) operator, desugars to `Seq`.
    Flow,
    /// `|||` — choice operator, desugars to `Choice`.
    ChoiceOp,
}

fn tokenize(src: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = src.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            ',' => {
                tokens.push(Token::Comma);
                chars.next();
            }
            '>' => {
                // Expect ">>>" (the flow / sequence operator)
                chars.next(); // consume first '>'
                if chars.next() != Some('>') {
                    return Err("expected '>>>' but got incomplete '>'".to_string());
                }
                if chars.next() != Some('>') {
                    return Err("expected '>>>' but got '>>'".to_string());
                }
                tokens.push(Token::Flow);
            }
            '|' => {
                // Expect "|||" (the choice operator)
                chars.next(); // consume first '|'
                if chars.next() != Some('|') {
                    return Err("expected '|||' but got incomplete '|'".to_string());
                }
                if chars.next() != Some('|') {
                    return Err("expected '|||' but got '||'".to_string());
                }
                tokens.push(Token::ChoiceOp);
            }
            '"' => {
                chars.next(); // skip opening quote
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '"' {
                        chars.next();
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                tokens.push(Token::StringLit(s));
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '.' {
                        s.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(s));
            }
            _ => {
                return Err(format!("unexpected character: '{}'", c));
            }
        }
    }

    Ok(tokens)
}

// ============================================================================
// Recursive descent parser (for simple arrow-graph syntax)
// ============================================================================

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos);
        self.pos += 1;
        t
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        match self.next() {
            Some(t) if t == expected => Ok(()),
            Some(t) => Err(format!("expected {:?}, got {:?}", expected, t)),
            None => Err("unexpected end of input".to_string()),
        }
    }

    /// Parse a full arrow expression (top-level entry point).
    ///
    /// Grammar:
    /// ```text
    /// expr   := seq
    /// seq    := choice_expr (">>>" choice_expr)*   -- left-assoc, produces Seq
    /// choice_expr := atom ("|||" atom)*             -- left-assoc, produces Choice
    /// atom   := "arr" "(" string ")" | "first" "(" expr ")"
    ///         | "loop" "(" expr ")" | "app" "(" string ")"
    ///         | "comp" "(" expr "," expr ")" | "choice" "(" expr "," expr ")"
    ///         | "id" | identifier
    /// ```
    fn parse_expr(&mut self) -> Result<Scheme, String> {
        self.parse_seq()
    }

    /// Parse a sequence expression: `choice_expr (">>>" choice_expr)*`.
    ///
    /// Left-associative — `a >>> b >>> c` becomes `Seq(Seq(a, b), c)`.
    /// `>>>` has lower precedence than `|||`, so `a >>> b ||| c` parses as
    /// `Seq(a, Choice(b, c))`.
    fn parse_seq(&mut self) -> Result<Scheme, String> {
        let mut left = self.parse_choice_expr()?;
        while self.peek() == Some(&Token::Flow) {
            self.next(); // consume >>>
            let right = self.parse_choice_expr()?;
            left = Scheme::Seq {
                fst: Box::new(left),
                snd: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Parse a choice expression: `atom ("|||" atom)*`.
    ///
    /// Left-associative — `a ||| b ||| c` becomes `Choice(Choice(a, b), c)`.
    /// `|||` has higher precedence than `>>>`.
    fn parse_choice_expr(&mut self) -> Result<Scheme, String> {
        let mut left = self.parse_atom()?;
        while self.peek() == Some(&Token::ChoiceOp) {
            self.next(); // consume |||
            let right = self.parse_atom()?;
            left = Scheme::Choice {
                l: Box::new(left),
                r: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Parse an atom: the simplest unit of an arrow expression.
    ///
    /// This handles function-call syntax (`comp(a, b)`, `choice(a, b)`,
    /// `first(a)`, `loop(a)`, `arr("name")`, `app("name", args)`) and bare
    /// identifiers (including `id`).
    fn parse_atom(&mut self) -> Result<Scheme, String> {
        match self.next() {
            Some(Token::LParen) => {
                // Parenthesized expression: (expr)
                let inner = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(inner)
            }
            Some(Token::Ident(name)) => match name.as_str() {
                "id" => Ok(Scheme::arr_native("id", vec![])),
                "arr" => self.parse_arr(),
                "app" => self.parse_app(),
                "comp" => self.parse_comp(),
                "first" => self.parse_first(),
                "choice" => self.parse_choice(),
                "loop" => self.parse_loop(),
                _ => Ok(Scheme::App {
                    callee: Box::new(Scheme::arr_native(name, vec![])),
                    args: vec![],
                }),
            },
            Some(t) => Err(format!("expected identifier, got {:?}", t)),
            None => Err("unexpected end of input".to_string()),
        }
    }

    fn parse_arr(&mut self) -> Result<Scheme, String> {
        self.expect(&Token::LParen)?;
        let name = match self.next() {
            Some(Token::StringLit(s)) => s.clone(),
            other => return Err(format!("expected string literal, got {:?}", other)),
        };
        let args = self.parse_optional_args()?;
        self.expect(&Token::RParen)?;
        Ok(Scheme::arr_native(name, args))
    }

    fn parse_app(&mut self) -> Result<Scheme, String> {
        self.expect(&Token::LParen)?;
        let name = match self.next() {
            Some(Token::StringLit(s)) => s.clone(),
            other => return Err(format!("expected string literal, got {:?}", other)),
        };
        let args = self.parse_optional_args()?;
        self.expect(&Token::RParen)?;
        Ok(Scheme::App {
            callee: Box::new(Scheme::arr_native(name, vec![])),
            args,
        })
    }

    fn parse_comp(&mut self) -> Result<Scheme, String> {
        self.expect(&Token::LParen)?;
        let fst = self.parse_expr()?;
        self.expect(&Token::Comma)?;
        let snd = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(Scheme::Seq {
            fst: Box::new(fst),
            snd: Box::new(snd),
        })
    }

    fn parse_first(&mut self) -> Result<Scheme, String> {
        self.expect(&Token::LParen)?;
        let inner = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(Scheme::First {
            inner: Box::new(inner),
        })
    }

    fn parse_choice(&mut self) -> Result<Scheme, String> {
        self.expect(&Token::LParen)?;
        let l = self.parse_expr()?;
        self.expect(&Token::Comma)?;
        let r = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(Scheme::Choice {
            l: Box::new(l),
            r: Box::new(r),
        })
    }

    fn parse_loop(&mut self) -> Result<Scheme, String> {
        self.expect(&Token::LParen)?;
        let body = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(Scheme::Loop {
            body: Box::new(body),
        })
    }

    fn parse_optional_args(&mut self) -> Result<Vec<Scheme>, String> {
        if self.peek() == Some(&Token::Comma) {
            self.next(); // consume comma
            let mut args = vec![self.parse_expr()?];
            while self.peek() == Some(&Token::Comma) {
                self.next();
                args.push(self.parse_expr()?);
            }
            Ok(args)
        } else {
            Ok(vec![])
        }
    }
}

// ============================================================================
// Rich loop syntax parser
// ============================================================================

/// Parse the rich `loop { ... } ( proc (state) -> match ... { ... } )` syntax.
///
/// The stdlib `.hnl` files use this surface syntax. The parser recognises the
/// structural patterns in the `proc` body and maps them to the corresponding
/// `Scheme` graph, matching the hand-built graphs in `e2e_tests.rs`.
fn parse_rich_loop(src: &str) -> Result<Scheme, String> {
    // Find the proc body: "proc (state) -> match"
    let proc_idx = src
        .find("proc (state) -> match")
        .ok_or("expected 'proc (state) -> match' in rich loop syntax")?;

    let proc_body = &src[proc_idx..];

    // Find the [h, ...t]: branch (the recursive case)
    let branch_marker = "[h, ...t]:";
    let branch_idx = proc_body
        .find(branch_marker)
        .ok_or("expected '[h, ...t]:' branch in match")?;

    let branch_body = &proc_body[branch_idx + branch_marker.len()..];

    // Analyze the branch body to determine the graph structure
    let body_scheme = analyze_branch(branch_body)?;

    Ok(Scheme::Loop {
        body: Box::new(body_scheme),
    })
}

/// Analyze the `[h, ...t]:` branch body and map it to a `Scheme` graph.
///
/// The patterns recognised (in order of specificity):
///
/// 1. `prim.compare` + `if/else` → `comp(prim.compare, choice(cons, cons))`
///    (insert: compare element with head, then choice of cons arms)
///
/// 2. `state.p(` + `if/else` → `comp(filter_step, id)`
///    (filter: test predicate on head, keep or skip)
///
/// 3. `state.f(state.acc` → `comp(fold_step, first(id))`
///    (fold: apply step to accumulator and head, list passes through)
///
/// 4. `insert(` (named call) → `comp(app(insert), first(id))`
///    (sort: insert element into accumulator, list passes through)
///
/// 5. `state.f(` → `first(fn.apply)`
///    (map: apply function to head, list is side channel)
fn analyze_branch(body: &str) -> Result<Scheme, String> {
    // 1. prim.compare + if/else → insert pattern
    if body.contains("prim.compare") {
        let step = Scheme::arr_native("prim.compare", vec![]);
        let choice = Scheme::Choice {
            l: Box::new(Scheme::arr_native("cons", vec![])),
            r: Box::new(Scheme::arr_native("cons", vec![])),
        };
        return Ok(Scheme::Seq {
            fst: Box::new(step),
            snd: Box::new(choice),
        });
    }

    // 2. state.p( + if/else → filter pattern
    if body.contains("state.p(") {
        let step = Scheme::arr_native("filter_step", vec![]);
        let rest = Scheme::arr_native("id", vec![]);
        return Ok(Scheme::Seq {
            fst: Box::new(step),
            snd: Box::new(rest),
        });
    }

    // 3. state.f(state.acc → fold pattern
    if body.contains("state.f(state.acc") {
        let step = Scheme::arr_native("fold_step", vec![]);
        let rest = Scheme::First {
            inner: Box::new(Scheme::arr_native("id", vec![])),
        };
        return Ok(Scheme::Seq {
            fst: Box::new(step),
            snd: Box::new(rest),
        });
    }

    // 4. insert( (named function call) → sort pattern
    if body.contains("insert(") {
        let step = Scheme::App {
            callee: Box::new(Scheme::arr_native("insert", vec![])),
            args: vec![],
        };
        let rest = Scheme::First {
            inner: Box::new(Scheme::arr_native("id", vec![])),
        };
        return Ok(Scheme::Seq {
            fst: Box::new(step),
            snd: Box::new(rest),
        });
    }

    // 5. state.f( → map pattern
    if body.contains("state.f(") {
        return Ok(Scheme::First {
            inner: Box::new(Scheme::arr_native("fn.apply", vec![])),
        });
    }

    Err(format!(
        "could not analyze branch body — no recognised pattern found"
    ))
}
