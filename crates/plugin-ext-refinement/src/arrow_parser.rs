//! A minimal arrow-graph parser for `.hnl` arrow files (ADR-0069).
//!
//! This parser reads the arrow-graph syntax used in `kit-stdlib/data/arrows/`
//! and produces `Scheme` IR. It uses the **data-driven** lexer + Pratt parser
//! (driven by the YAML language definition in `data/`) to build an AST, then
//! materializes the AST into a `Scheme` via [`ast_to_scheme`].
//!
//! # Two syntaxes
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
//!    actually used in the stdlib `.hnl` files. The YAML grammar tokenizes
//!    and parses this syntax into a `loop_rich` AST node. The
//!    `ast_to_scheme` materializer then pattern-matches on the proc body's
//!    recursive match arm to determine the corresponding `Scheme` graph —
//!    the same heuristic as the legacy ad-hoc parser, but operating on the
//!    AST instead of raw text.
//!
//! # Fallback strategy
//!
//! The data-driven parser is tried first. If it fails (e.g. for unsupported
//! constructs), the legacy ad-hoc recursive-descent parser is used as a
//! fallback. This ensures existing `.hnl` files continue to parse correctly.
//! The simple `loop(body)` form now falls through to the legacy parser
//! because the `loop` prefix grammar row was changed to handle the rich
//! `loop { ... } ( ... )` form (arity 2 instead of 1).

use std::collections::{HashMap, HashSet};

use handl_language::{Ast, AstNode, Language, Scheme, Value};
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;

use crate::arrow_lang::arrow_language;

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
/// Tries the data-driven parser first (lex + Pratt parse + materialize),
/// falling back to the legacy ad-hoc parser on failure. Rich-loop syntax
/// (`loop { ... } ( proc ... )`) is handled by the data-driven parser:
/// the YAML grammar tokenizes and parses it into a `loop_rich` AST node,
/// which `ast_to_scheme` pattern-matches to determine the corresponding
/// `Scheme` graph.
pub fn parse_arrow_file(src: &str) -> Result<ParsedFile, String> {
    let stripped = strip_comments(src);
    let trimmed = stripped.trim();

    if trimmed.is_empty() {
        return Err("empty file (after stripping comments)".to_string());
    }

    // Try the data-driven parser first.
    if let Ok(result) = parse_arrow_file_data_driven(&stripped) {
        return Ok(result);
    }

    // Fall back to the legacy ad-hoc parser.
    parse_arrow_file_legacy(&stripped)
}

/// Parse a simple arrow expression (the combinator syntax).
///
/// Tries the data-driven parser first, falling back to the legacy ad-hoc
/// parser on failure.
///
/// Handles: `arr("name")`, `comp(a, b)`, `first(a)`, `choice(a, b)`,
/// `loop(a)`, `id`, `app("name", args)`, and bare identifiers (references
/// to named arrows). Also supports infix operator syntax: `a >>> b`
/// (sequence, same as `comp(a, b)`) and `a ||| b` (choice, same as
/// `choice(a, b)`).
pub fn parse_arrow_expr(src: &str) -> Result<Scheme, String> {
    let lang = arrow_language();
    if let Ok(scheme) = parse_arrow_expr_data_driven(src, lang) {
        return Ok(scheme);
    }
    parse_arrow_expr_legacy(src)
}

// ============================================================================
// Data-driven parsing (lex + Pratt + materialize)
// ============================================================================

/// Parse an arrow file using the data-driven parser.
///
/// Dispatches to named-arrow parsing (multi-line `name = expr`) or
/// single-expression parsing. Rich-loop syntax (`loop { ... }`) is always
/// treated as a single expression (not named arrows) because the proc body
/// may contain ` = ` in let-bindings that would falsely trigger the
/// named-arrow heuristic.
fn parse_arrow_file_data_driven(src: &str) -> Result<ParsedFile, String> {
    let lang = arrow_language();
    let trimmed = src.trim();

    // Rich-loop syntax starts with `loop` followed by `{`. Parse it as a
    // single expression — do NOT check for named arrows, because the proc
    // body may contain ` = ` in let-bindings (`cmp = expr in body`).
    if is_rich_loop(trimmed) {
        let scheme = parse_arrow_expr_data_driven(trimmed, lang)?;
        return Ok(ParsedFile::Single(scheme));
    }

    // Named arrows: lines with ` = `
    if has_named_arrows(trimmed) {
        let map = parse_named_arrows_data_driven(trimmed, lang)?;
        return Ok(ParsedFile::Named(map));
    }

    // Single expression
    let scheme = parse_arrow_expr_data_driven(trimmed, lang)?;
    Ok(ParsedFile::Single(scheme))
}

/// Check if the source is a rich-loop expression (`loop { ... }`).
///
/// Returns true if the trimmed source starts with the `loop` keyword
/// followed by `{` (with possible whitespace between). This distinguishes
/// the rich-loop form from the simple `loop(...)` form.
fn is_rich_loop(src: &str) -> bool {
    let trimmed = src.trim_start();
    if let Some(rest) = trimmed.strip_prefix("loop") {
        rest.trim_start().starts_with('{')
    } else {
        false
    }
}

/// Parse a single arrow expression using the data-driven lexer + Pratt parser.
fn parse_arrow_expr_data_driven(src: &str, lang: &Language) -> Result<Scheme, String> {
    let tokens = handl_plugin_engine_lex_pattern::lex(lang, src)
        .map_err(|e| format!("lex error: {}", e))?;

    if tokens.0.is_empty() {
        return Err("no tokens produced by lexer".to_string());
    }

    let engine = PrattEngine::new();
    let req = ParserRequest::new(tokens);
    let ast = engine
        .parse(lang, &req)
        .map_err(|e| format!("parse error: {}", e))?;

    ast_to_scheme(&ast, lang)
}

/// Parse named arrows (`name = expr` lines) using the data-driven parser.
///
/// Splits the source into lines, and for each line containing ` = `, parses
/// the expression part with the data-driven parser.
fn parse_named_arrows_data_driven(
    src: &str,
    lang: &Language,
) -> Result<HashMap<String, Scheme>, String> {
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
            let scheme = parse_arrow_expr_data_driven(expr_str, lang)?;
            map.insert(name, scheme);
        }
    }

    if map.is_empty() {
        return Err("no named arrows found".to_string());
    }

    Ok(map)
}

// ============================================================================
// AST-to-Scheme materializer
// ============================================================================

/// Materialize an [`Ast`] (produced by the Pratt parser) into a [`Scheme`].
///
/// The root node is the last node in the arena (Pratt convention: children
/// are pushed before parents). For each node, the eval scheme is looked up
/// via `lang.eval.get_by_identity(&node.kind)`, then walked to produce a
/// concrete `Scheme` with `proj_child(N)` placeholders replaced by the
/// materialized Nth child.
///
/// # Special placeholders
///
/// - `proj_child` (with `lit: N`): recursively materialize child N.
/// - `$str_child0` (as `op`): the op name is child 0's payload string
///   (used by `arr_call` / `app_call` for `arr("name")`).
/// - `$payload` (as `op`): the op name is the current node's payload string
///   (used by `arrow_var` for bare identifiers).
/// - `proj_rest` (in `args`): expand to the materialized children 1..N
///   (the argument expressions after the string name in `arr(...)` / `app(...)`).
/// - `$payload` (as `lit`): the literal value is the node's payload.
///
/// # Errors
///
/// Returns a `String` error if a node kind has no eval scheme, a placeholder
/// cannot be resolved, or a child index is out of bounds.
pub fn ast_to_scheme(ast: &Ast, lang: &Language) -> Result<Scheme, String> {
    if ast.is_empty() {
        return Err("empty AST — no root node".to_string());
    }
    // Root is the last node pushed (Pratt convention).
    let root_idx = ast.len() - 1;
    materialize_node(ast, root_idx, lang)
}

/// Materialize the node at arena index `idx` into a [`Scheme`].
fn materialize_node(ast: &Ast, idx: usize, lang: &Language) -> Result<Scheme, String> {
    let node = ast.nodes().get(idx).ok_or_else(|| {
        format!(
            "materialize_node: arena index {} out of bounds (len {})",
            idx,
            ast.len()
        )
    })?;
    let kind = &node.kind;

    // Special-case `named_arrow`: the eval scheme uses `extend_env` for
    // runtime evaluation, but for static materialization we just materialize
    // the expression (child 1). The name binding is handled at the file level.
    if kind == "named_arrow" {
        let children = get_children(&node.payload)?;
        let expr_idx = children
            .get(1)
            .and_then(|v| v.as_u64())
            .ok_or("named_arrow: missing expression child")? as usize;
        return materialize_node(ast, expr_idx, lang);
    }

    // Special-case `loop_rich`: the rich-loop syntax is SUGAR. The YAML
    // grammar parses it into a `loop_rich` AST node, but the actual `Scheme`
    // graph is determined by pattern-matching on the proc body structure
    // (which primitives are called, what the state transitions are) — the
    // same heuristic as the legacy `analyze_branch`, but operating on the
    // AST instead of raw text.
    if kind == "loop_rich" {
        return rich_loop_to_scheme(ast, node);
    }

    // Look up the eval scheme for this node kind.
    let eval_def = lang
        .eval
        .get_by_identity(kind)
        .ok_or_else(|| format!("no eval scheme for node kind: {}", kind))?;

    walk_scheme(&eval_def.scheme, ast, node, lang)
}

/// Walk a [`Scheme`] template (from the eval definition), substituting
/// `proj_child(N)` placeholders with materialized children and resolving
/// `$str_child0` / `$payload` / `proj_rest` conventions.
fn walk_scheme(
    scheme: &Scheme,
    ast: &Ast,
    node: &AstNode,
    lang: &Language,
) -> Result<Scheme, String> {
    match scheme {
        Scheme::Arr {
            op,
            args,
            source,
            lit,
        } => match op.as_str() {
            "proj_child" => {
                // Recurse into child N of the current node.
                let idx = lit
                    .as_ref()
                    .and_then(|v| v.as_u64())
                    .ok_or("proj_child: missing index literal")? as usize;
                let children = get_children(&node.payload)?;
                let child_arena_idx = children
                    .get(idx)
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| format!("proj_child: child {} not found", idx))? as usize;
                materialize_node(ast, child_arena_idx, lang)
            }
            "$str_child0" => {
                // The op name is child 0's payload string (arr("name") / app("name")).
                let children = get_children(&node.payload)?;
                let child0_idx = children
                    .first()
                    .and_then(|v| v.as_u64())
                    .ok_or("$str_child0: missing child 0")? as usize;
                let child0 = ast
                    .nodes()
                    .get(child0_idx)
                    .ok_or("$str_child0: child 0 out of bounds")?;
                let name = child0
                    .payload
                    .as_str()
                    .ok_or("$str_child0: child 0 payload is not a string")?
                    .to_string();
                let materialized_args = walk_args(args, ast, node, lang)?;
                Ok(Scheme::Arr {
                    op: name,
                    args: materialized_args,
                    source: source.clone(),
                    lit: None,
                })
            }
            "$payload" => {
                // The op name is the current node's payload string (bare identifier).
                let name = node
                    .payload
                    .as_str()
                    .ok_or("$payload: node payload is not a string")?
                    .to_string();
                let materialized_args = walk_args(args, ast, node, lang)?;
                Ok(Scheme::Arr {
                    op: name,
                    args: materialized_args,
                    source: source.clone(),
                    lit: None,
                })
            }
            "proj_rest" => {
                // proj_rest should only appear inside an args list (handled by
                // walk_args). If reached here, it's a malformed eval scheme.
                Err("proj_rest: should only appear in an args list".to_string())
            }
            _ => {
                // Normal arr: walk args recursively, resolve $payload in lit.
                let materialized_args = walk_args(args, ast, node, lang)?;
                let materialized_lit = resolve_lit(lit, node);
                Ok(Scheme::Arr {
                    op: op.clone(),
                    args: materialized_args,
                    source: source.clone(),
                    lit: materialized_lit,
                })
            }
        },

        Scheme::Seq { fst, snd } => Ok(Scheme::Seq {
            fst: Box::new(walk_scheme(fst, ast, node, lang)?),
            snd: Box::new(walk_scheme(snd, ast, node, lang)?),
        }),

        Scheme::First { inner } => Ok(Scheme::First {
            inner: Box::new(walk_scheme(inner, ast, node, lang)?),
        }),

        Scheme::Choice { l, r } => Ok(Scheme::Choice {
            l: Box::new(walk_scheme(l, ast, node, lang)?),
            r: Box::new(walk_scheme(r, ast, node, lang)?),
        }),

        Scheme::Loop { body } => Ok(Scheme::Loop {
            body: Box::new(walk_scheme(body, ast, node, lang)?),
        }),

        Scheme::App { callee, args } => {
            let materialized_callee = walk_scheme(callee, ast, node, lang)?;
            let materialized_args = walk_args(args, ast, node, lang)?;
            Ok(Scheme::App {
                callee: Box::new(materialized_callee),
                args: materialized_args,
            })
        }
    }
}

/// Walk a list of argument schemes, expanding `proj_rest` placeholders into
/// the materialized children 1..N of the current node.
fn walk_args(
    args: &[Scheme],
    ast: &Ast,
    node: &AstNode,
    lang: &Language,
) -> Result<Vec<Scheme>, String> {
    let mut result = Vec::new();
    for arg in args {
        if let Scheme::Arr { op, .. } = arg {
            if op == "proj_rest" {
                // Expand to materialized children 1..N (all children after
                // the string-name child 0).
                let children = get_children(&node.payload)?;
                for i in 1..children.len() {
                    let child_idx = children[i]
                        .as_u64()
                        .ok_or("proj_rest: child index not a number")? as usize;
                    result.push(materialize_node(ast, child_idx, lang)?);
                }
                continue;
            }
        }
        result.push(walk_scheme(arg, ast, node, lang)?);
    }
    Ok(result)
}

/// Resolve a `lit` payload that may contain the `$payload` placeholder.
///
/// If `lit` is the string `"$payload"`, it is replaced with the node's actual
/// payload value. Otherwise, the literal is passed through unchanged.
fn resolve_lit(lit: &Option<Value>, node: &AstNode) -> Option<Value> {
    lit.as_ref().and_then(|v| {
        if v == &Value::String("$payload".to_string()) {
            Some(node.payload.clone())
        } else {
            Some(v.clone())
        }
    })
}

/// Extract the `children` array from a node's payload.
fn get_children(payload: &Value) -> Result<&Vec<Value>, String> {
    payload
        .get("children")
        .and_then(|c| c.as_array())
        .ok_or("expected 'children' array in node payload".to_string())
}

// ============================================================================
// Rich-loop AST-to-Scheme conversion (pattern matching on proc body)
// ============================================================================
//
// The rich-loop syntax (`loop { ... } ( proc (state) -> match ... )`) is
// SUGAR. The YAML grammar tokenizes and parses it into a `loop_rich` AST
// node, but the actual `Scheme` graph is determined by recognizing the
// structural patterns in the `proc` body's recursive match arm — the same
// heuristic as the legacy `analyze_branch`, but operating on the AST
// instead of raw text.
//
// AST structure of a rich-loop:
//   loop_rich (children: [decl_block, proc_expr])
//     └─ proc_expr: arrow_proc (children: [params, body])
//        └─ body: arrow_match (children: [scrutinee, arms_container])
//           └─ arms_container: block (children: [arm1, arm2, ...])
//              ├─ arm1: arrow_arm (children: [pattern, body])  — base case
//              │   └─ pattern: arrow_array (children: [])      — `[]`
//              └─ arm2: arrow_arm (children: [pattern, body])  — recursive case
//                  └─ pattern: arrow_array (children: [...])   — `[h, ...t]`
//                  └─ body: ...                                 — recursive body

/// Convert a `loop_rich` AST node into a [`Scheme`] by pattern-matching on
/// the proc body's recursive match arm.
///
/// This navigates the AST to find the `[h, ...t]:` arm (the recursive case),
/// then calls [`analyze_branch_ast`] to determine which `Scheme` graph the
/// arm body corresponds to.
fn rich_loop_to_scheme(ast: &Ast, node: &AstNode) -> Result<Scheme, String> {
    let children = get_children(&node.payload)?;

    // Child 1 is the proc expression (child 0 is the declaration block).
    let proc_idx = children
        .get(1)
        .and_then(|v| v.as_u64())
        .ok_or("loop_rich: missing proc child")? as usize;
    let proc_node = ast
        .nodes()
        .get(proc_idx)
        .ok_or("loop_rich: proc node out of bounds")?;

    if proc_node.kind != "arrow_proc" {
        return Err(format!(
            "loop_rich: expected arrow_proc, got {}",
            proc_node.kind
        ));
    }

    let proc_children = get_children(&proc_node.payload)?;
    // Child 1 of proc is the body (child 0 is params).
    let body_idx = proc_children
        .get(1)
        .and_then(|v| v.as_u64())
        .ok_or("loop_rich: missing proc body")? as usize;
    let body_node = ast
        .nodes()
        .get(body_idx)
        .ok_or("loop_rich: body node out of bounds")?;

    if body_node.kind != "arrow_match" {
        return Err(format!(
            "loop_rich: expected arrow_match body, got {}",
            body_node.kind
        ));
    }

    let match_children = get_children(&body_node.payload)?;
    // Child 1 of match is the arms container (child 0 is scrutinee).
    let arms_idx = match_children
        .get(1)
        .and_then(|v| v.as_u64())
        .ok_or("loop_rich: missing arms container")? as usize;
    let arms_node = ast
        .nodes()
        .get(arms_idx)
        .ok_or("loop_rich: arms node out of bounds")?;

    let arms_children = get_children(&arms_node.payload)?;

    // Find the arm with a non-empty array pattern (the recursive case).
    let mut recursive_body_idx: Option<usize> = None;
    for arm_idx_val in arms_children {
        let arm_idx = arm_idx_val
            .as_u64()
            .ok_or("loop_rich: arm index not a number")? as usize;
        let arm_node = ast
            .nodes()
            .get(arm_idx)
            .ok_or("loop_rich: arm node out of bounds")?;
        if arm_node.kind != "arrow_arm" {
            continue;
        }
        let arm_children = get_children(&arm_node.payload)?;
        let pattern_idx = arm_children
            .first()
            .and_then(|v| v.as_u64())
            .ok_or("loop_rich: missing pattern")? as usize;
        let pattern_node = ast
            .nodes()
            .get(pattern_idx)
            .ok_or("loop_rich: pattern node out of bounds")?;

        // Check if the pattern is a non-empty array (`[h, ...t]`).
        if pattern_node.kind == "arrow_array" {
            if let Ok(pattern_children) = get_children(&pattern_node.payload) {
                if !pattern_children.is_empty() {
                    // This is the recursive case arm.
                    let arm_body_idx = arm_children
                        .get(1)
                        .and_then(|v| v.as_u64())
                        .ok_or("loop_rich: missing arm body")? as usize;
                    recursive_body_idx = Some(arm_body_idx);
                    break;
                }
            }
        }
    }

    let body_idx = recursive_body_idx
        .ok_or("loop_rich: no recursive case arm found")?;

    // Analyze the recursive body AST to determine the Scheme graph.
    let body_scheme = analyze_branch_ast(ast, body_idx)?;

    Ok(Scheme::Loop {
        body: Box::new(body_scheme),
    })
}

/// Analyze the recursive case arm body AST and map it to a [`Scheme`] graph.
///
/// This mirrors the legacy `analyze_branch` heuristic but operates on the AST
/// instead of raw text. The patterns recognized (in order of specificity):
///
/// 0. Nested `loop_rich` → recursively materialize the inner loop
/// 1. `prim.compare` identifier → `comp(prim.compare, choice(cons, cons))` (insert)
/// 2. `*.p` in arrow_app (`-<`) → `comp(filter_step, id)` (filter)
/// 3. `*.f` in call with `*.acc` argument → `comp(fold_step, first(id))` (fold)
/// 4. `insert` in arrow_app (`-<`) → `comp(app(insert), first(id))` (sort)
/// 5. `*.f` in arrow_app with `*.acc` in right args → `comp(fold_step, first(list.cons))` (scan)
/// 6. `list.head` AND `list.tail` both in arrow_call → `comp(first(list.head), comp(first(list.tail), list.cons))` (zip)
/// 7. `list.head` in arrow_app (`-<`) → `comp(list.head, choice(cons, id))` (take)
/// 8. `list.tail` in arrow_call → `comp(list.tail, first(id))` (drop)
/// 9. `*.f` in arrow_app (`-<`) → `first(fn.apply)` (map)
///
/// Patterns 2, 3, 5, and 9 use suffix-based matching (e.g. `*.f` matches
/// `state.f`, `inner_state.f`, etc.) so that nested loops with different
/// state variable names are recognized correctly.
///
/// Early-exit patterns (a `match` arm that returns without recursing) are
/// inherently handled: the base-case arm (`[]:`) is skipped, and only the
/// recursive arm (`[h, ...t]:`) is analyzed. Conditionals within the
/// recursive arm (e.g. `if state.n <= 0 then returnA ...`) are tracked via
/// `has_conditional` for future pattern discrimination.
fn analyze_branch_ast(ast: &Ast, root_idx: usize) -> Result<Scheme, String> {
    let mut collector = PatternCollector::default();
    collector.collect(ast, root_idx);

    // 0. Nested loop → recursively materialize the inner loop_rich node.
    //    If the recursive arm body contains a nested `loop { ... }`, we find
    //    it and recursively call `rich_loop_to_scheme` to materialize it.
    //    Note: `rich_loop_to_scheme` already wraps the result in `Scheme::Loop`,
    //    and the outer `rich_loop_to_scheme` will wrap this result again,
    //    producing the correct `Loop(Loop(...))` structure.
    if collector.has_nested_loop {
        if let Some(nested_idx) = find_node_kind(ast, root_idx, "loop_rich") {
            let nested_node = ast
                .nodes()
                .get(nested_idx)
                .ok_or("nested loop_rich node out of bounds")?;
            return rich_loop_to_scheme(ast, nested_node);
        }
    }

    // Helper: check if any name in a set ends with the given suffix.
    // This allows matching `state.f`, `inner_state.f`, etc.
    let has_suffix = |set: &HashSet<String>, suffix: &str| {
        set.iter().any(|name| name.ends_with(suffix))
    };

    // 1. prim.compare → insert pattern
    if collector.var_names.contains("prim.compare") {
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

    // 2. *.p in arrow_app → filter pattern
    if has_suffix(&collector.app_left_names, ".p") {
        let step = Scheme::arr_native("filter_step", vec![]);
        let rest = Scheme::arr_native("id", vec![]);
        return Ok(Scheme::Seq {
            fst: Box::new(step),
            snd: Box::new(rest),
        });
    }

    // 3. *.f in call with *.acc argument → fold pattern
    for (callee, args) in &collector.call_callee_args {
        if callee.ends_with(".f") && args.iter().any(|a| a.ends_with(".acc")) {
            let step = Scheme::arr_native("fold_step", vec![]);
            let rest = Scheme::First {
                inner: Box::new(Scheme::arr_native("id", vec![])),
            };
            return Ok(Scheme::Seq {
                fst: Box::new(step),
                snd: Box::new(rest),
            });
        }
    }

    // 4. insert in arrow_app → sort pattern
    if collector.app_left_names.contains("insert") {
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

    // 5. *.f in arrow_app with *.acc in right args → scan pattern
    //    scan is like fold (uses f with acc) but also conses
    //    each intermediate result onto an output list.
    for (left, right_args) in &collector.app_left_args {
        if left.ends_with(".f") && right_args.iter().any(|a| a.ends_with(".acc")) {
            let step = Scheme::arr_native("fold_step", vec![]);
            let rest = Scheme::First {
                inner: Box::new(Scheme::arr_native("list.cons", vec![])),
            };
            return Ok(Scheme::Seq {
                fst: Box::new(step),
                snd: Box::new(rest),
            });
        }
    }

    // 6. list.head AND list.tail both in arrow_call → zip pattern.
    //    Zip pairs two lists: it takes the head of both lists (one via
    //    pattern destructuring, the other via list.head() call), advances
    //    both (one via pattern tail, the other via list.tail() call), and
    //    conses the pair onto the output via list.cons.
    //    Graph: first(list.head) >>> (first(list.tail) >>> list.cons)
    //    This must be checked BEFORE the take and drop patterns (7 and 8)
    //    because zip uses list.tail in arrow_call (which would match drop)
    //    and the combination of both list.head and list.tail in arrow_call
    //    is unique to zip.
    if collector.call_callee_args.contains_key("list.head")
        && collector.call_callee_args.contains_key("list.tail")
    {
        let step1 = Scheme::First {
            inner: Box::new(Scheme::arr_native("list.head", vec![])),
        };
        let step2 = Scheme::First {
            inner: Box::new(Scheme::arr_native("list.tail", vec![])),
        };
        let cons = Scheme::arr_native("list.cons", vec![]);
        return Ok(Scheme::Seq {
            fst: Box::new(step1),
            snd: Box::new(Scheme::Seq {
                fst: Box::new(step2),
                snd: Box::new(cons),
            }),
        });
    }

    // 7. list.head in arrow_app → take pattern
    if collector.app_left_names.contains("list.head") {
        let step = Scheme::arr_native("list.head", vec![]);
        let choice = Scheme::Choice {
            l: Box::new(Scheme::arr_native("cons", vec![])),
            r: Box::new(Scheme::arr_native("id", vec![])),
        };
        return Ok(Scheme::Seq {
            fst: Box::new(step),
            snd: Box::new(choice),
        });
    }

    // 8. list.tail in arrow_call → drop pattern
    if collector.call_callee_args.contains_key("list.tail") {
        let step = Scheme::arr_native("list.tail", vec![]);
        let rest = Scheme::First {
            inner: Box::new(Scheme::arr_native("id", vec![])),
        };
        return Ok(Scheme::Seq {
            fst: Box::new(step),
            snd: Box::new(rest),
        });
    }

    // 9. *.f in arrow_app → map pattern
    if has_suffix(&collector.app_left_names, ".f") {
        return Ok(Scheme::First {
            inner: Box::new(Scheme::arr_native("fn.apply", vec![])),
        });
    }

    Err("could not analyze branch body — no recognised pattern found".to_string())
}

/// Recursively collects structural patterns from the AST subtree for the
/// rich-loop heuristic.
#[derive(Default)]
struct PatternCollector {
    /// All `arrow_var` identifier names found in the subtree.
    var_names: HashSet<String>,
    /// Left-child names of `arrow_app` (`-<`) nodes where the left child is
    /// an `arrow_var` (e.g. `state.f -< h` → `"state.f"`).
    app_left_names: HashSet<String>,
    /// For each `arrow_app` (`-<`) node, maps the left-child name to the set
    /// of `arrow_var` names found in the right-child subtree (e.g.
    /// `state.f -< { acc: state.acc, h: h }` →
    /// `"state.f" → {"state.acc", "h"}`).
    app_left_args: HashMap<String, HashSet<String>>,
    /// Callee name → argument names for `arrow_call` nodes where the callee
    /// is an `arrow_var` (e.g. `state.f(state.acc, h)` →
    /// `"state.f" → ["state.acc", "h"]`).
    call_callee_args: HashMap<String, Vec<String>>,
    /// Whether the subtree contains an `arrow_if` node (conditional).
    has_conditional: bool,
    /// Whether the subtree contains an `arrow_array` with an `arrow_rest`
    /// child (i.e. a cons-like `[x, ...rest]` construction).
    has_array_spread: bool,
    /// Whether the subtree contains a nested `loop_rich` node.
    has_nested_loop: bool,
}

impl PatternCollector {
    /// Recursively walk the AST from `idx`, collecting patterns.
    fn collect(&mut self, ast: &Ast, idx: usize) {
        let node = match ast.nodes().get(idx) {
            Some(n) => n,
            None => return,
        };

        match node.kind.as_str() {
            "arrow_var" => {
                // Identifier — record its name.
                if let Some(name) = node.payload.as_str() {
                    self.var_names.insert(name.to_string());
                }
                // No children to recurse into.
                return;
            }
            "arrow_if" => {
                // Conditional (if/then/else) — record its presence.
                self.has_conditional = true;
            }
            "loop_rich" => {
                // Nested rich loop — record its presence.
                self.has_nested_loop = true;
            }
            "arrow_array" => {
                // Check for cons-like construction: `[x, ...rest]`.
                // An `arrow_rest` child indicates a spread/cons.
                if let Ok(children) = get_children(&node.payload) {
                    for child_val in children {
                        if let Some(child_idx) = child_val.as_u64() {
                            if let Some(child_node) = ast.nodes().get(child_idx as usize) {
                                if child_node.kind == "arrow_rest" {
                                    self.has_array_spread = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            "arrow_app" => {
                // `-<` application: left child is the arrow being applied.
                if let Ok(children) = get_children(&node.payload) {
                    if let Some(left_idx) = children.first().and_then(|v| v.as_u64()) {
                        if let Some(left_node) = ast.nodes().get(left_idx as usize) {
                            if left_node.kind == "arrow_var" {
                                if let Some(name) = left_node.payload.as_str() {
                                    self.app_left_names.insert(name.to_string());
                                    // Collect var names from the right child
                                    // (child 1) to track app arguments.
                                    if let Some(right_idx) =
                                        children.get(1).and_then(|v| v.as_u64())
                                    {
                                        let right_vars =
                                            collect_var_names(ast, right_idx as usize);
                                        self.app_left_args
                                            .entry(name.to_string())
                                            .or_default()
                                            .extend(right_vars);
                                    }
                                }
                            }
                        }
                    }
                    // Recurse into all children.
                    for child_val in children {
                        if let Some(child_idx) = child_val.as_u64() {
                            self.collect(ast, child_idx as usize);
                        }
                    }
                }
                return;
            }
            "arrow_call" => {
                // `callee(args...)` function call: child 0 is callee,
                // children 1..N are arguments.
                if let Ok(children) = get_children(&node.payload) {
                    if let Some(callee_idx) = children.first().and_then(|v| v.as_u64()) {
                        if let Some(callee_node) = ast.nodes().get(callee_idx as usize) {
                            if callee_node.kind == "arrow_var" {
                                if let Some(callee_name) = callee_node.payload.as_str() {
                                    let mut arg_names = Vec::new();
                                    for i in 1..children.len() {
                                        if let Some(arg_idx) = children[i].as_u64() {
                                            if let Some(arg_node) =
                                                ast.nodes().get(arg_idx as usize)
                                            {
                                                if arg_node.kind == "arrow_var" {
                                                    if let Some(arg_name) =
                                                        arg_node.payload.as_str()
                                                    {
                                                        arg_names.push(arg_name.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    self.call_callee_args
                                        .insert(callee_name.to_string(), arg_names);
                                }
                            }
                        }
                    }
                    // Recurse into all children.
                    for child_val in children {
                        if let Some(child_idx) = child_val.as_u64() {
                            self.collect(ast, child_idx as usize);
                        }
                    }
                }
                return;
            }
            _ => {}
        }

        // Default: recurse into children for all other node kinds.
        if let Ok(children) = get_children(&node.payload) {
            for child_val in children {
                if let Some(child_idx) = child_val.as_u64() {
                    self.collect(ast, child_idx as usize);
                }
            }
        }
    }
}

/// Collect all `arrow_var` identifier names in an AST subtree.
///
/// This is used by `PatternCollector` to gather the variable names on the
/// right-hand side of an `arrow_app` (`-<`) node.
fn collect_var_names(ast: &Ast, idx: usize) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_var_names_recursive(ast, idx, &mut names);
    names
}

fn collect_var_names_recursive(ast: &Ast, idx: usize, names: &mut HashSet<String>) {
    let node = match ast.nodes().get(idx) {
        Some(n) => n,
        None => return,
    };

    if node.kind == "arrow_var" {
        if let Some(name) = node.payload.as_str() {
            names.insert(name.to_string());
        }
        return;
    }

    if let Ok(children) = get_children(&node.payload) {
        for child_val in children {
            if let Some(child_idx) = child_val.as_u64() {
                collect_var_names_recursive(ast, child_idx as usize, names);
            }
        }
    }
}

/// Find the first AST node of the given `kind` in the subtree rooted at `idx`.
///
/// Used by `analyze_branch_ast` to locate nested `loop_rich` nodes for
/// recursive materialization.
fn find_node_kind(ast: &Ast, idx: usize, kind: &str) -> Option<usize> {
    let node = ast.nodes().get(idx)?;
    if node.kind == kind {
        return Some(idx);
    }
    if let Ok(children) = get_children(&node.payload) {
        for child_val in children {
            if let Some(child_idx) = child_val.as_u64() {
                if let Some(found) = find_node_kind(ast, child_idx as usize, kind) {
                    return Some(found);
                }
            }
        }
    }
    None
}

// ============================================================================
// Legacy ad-hoc parser (fallback)
// ============================================================================

/// Legacy: parse a `.hnl` arrow file using the ad-hoc recursive-descent parser.
fn parse_arrow_file_legacy(src: &str) -> Result<ParsedFile, String> {
    let trimmed = src.trim();

    if trimmed.is_empty() {
        return Err("empty file (after stripping comments)".to_string());
    }

    // Named arrows: lines with `name = expr`
    if has_named_arrows(trimmed) {
        let map = parse_named_arrows(trimmed)?;
        return Ok(ParsedFile::Named(map));
    }

    // Simple single-expression file
    let scheme = parse_arrow_expr_legacy(trimmed)?;
    Ok(ParsedFile::Single(scheme))
}

/// Legacy: parse a simple arrow expression using the ad-hoc tokenizer +
/// recursive-descent parser.
fn parse_arrow_expr_legacy(src: &str) -> Result<Scheme, String> {
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
// Named arrow detection and parsing (legacy)
// ============================================================================

/// Check if the source contains named arrow definitions (`name = expr`).
fn has_named_arrows(src: &str) -> bool {
    src.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && trimmed.contains(" = ") && !trimmed.starts_with("==")
    })
}

/// Legacy: parse named arrows: `name = expr` on each line.
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
            let scheme = parse_arrow_expr_legacy(expr_str)?;
            map.insert(name, scheme);
        }
    }

    if map.is_empty() {
        return Err("no named arrows found".to_string());
    }

    Ok(map)
}

// ============================================================================
// Tokenizer (for simple arrow-graph syntax — legacy fallback)
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
// Recursive descent parser (for simple arrow-graph syntax — legacy fallback)
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
    fn parse_expr(&mut self) -> Result<Scheme, String> {
        self.parse_seq()
    }

    /// Parse a sequence expression: `choice_expr (">>>" choice_expr)*`.
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
    fn parse_atom(&mut self) -> Result<Scheme, String> {
        match self.next() {
            Some(Token::LParen) => {
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
