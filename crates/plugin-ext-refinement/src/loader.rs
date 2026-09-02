//! Parser for handl contract files.
//!
//! Reads `.hnl` contract files and populates a `ContractRegistry`. This moves
//! axiomatic contracts out of Rust source (`contracts.rs`) and into handl
//! contract files, so the `ContractRegistry` becomes a loader that reads these
//! files at startup.
//!
//! # Contract file format
//!
//! Each axiom has the form:
//!
//! ```text
//! axiom <name> : <input-type> -> <output-type>
//!   requires: <predicate>      // optional, defaults to True
//!   guarantee: <predicate>     // required
//!   termination: <effect>      // optional
//! ```
//!
//! ## Type syntax
//!
//! ```text
//! Int                            -> Plain(Int)
//! Bool                           -> Plain(Bool)
//! a, b, c (type variables)       -> Plain(Var("a"))
//! List T                         -> List(T)
//! (T * T)                        -> Prod(T, T)
//! (T + T)                        -> Sum(T, T)
//! { x : T | pred }               -> Refine(T, pred)
//! ```
//!
//! ## Predicate syntax
//!
//! ```text
//! True                           -> True
//! False                          -> False
//! Sorted(x)                      -> Sorted("x")
//! LengthEq(a, b)                 -> LengthEq("a", "b")
//! LengthLe(a, b)                 -> LengthLe("a", "b")
//! AllSatisfy(list, pred)         -> AllSatisfy("list", "pred")
//! Le(a, b)                       -> Le("a", "b")
//! HeadOf(elem, list)             -> HeadOf("elem", "list")
//! LengthPlusOne(a, b)            -> LengthPlusOne("a", "b")
//! LengthSubOne(a, b)             -> LengthSubOne("a", "b")
//! IsPerm(a, b)                   -> IsPerm("a", "b")
//! p && q                         -> And(p, q)
//! p || q                         -> Or(p, q)
//! p ==> q                        -> Implies(p, q)
//! forall x in list, pred         -> Forall("x", "list", "pred")
//! ```
//!
//! The parser is self-contained (no external parsing crate): it uses simple
//! line-based scanning with a small recursive-descent expression parser for
//! types and predicates.

use std::path::Path;

use crate::contracts::ContractRegistry;
use crate::types::{BaseTy, Contract, Pred, RefTy};
use handl_language::ArrSource;

// Bundled contract files (compiled into the crate via `include_str!`).
const PRIM_CONTRACTS: &str = include_str!("../contracts/prim.hnl");
const LIST_CONTRACTS: &str = include_str!("../contracts/list.hnl");
const PARSE_CONTRACTS: &str = include_str!("../contracts/parse.hnl");
const STATE_CONTRACTS: &str = include_str!("../contracts/state.hnl");

// ============================================================================
// Public API
// ============================================================================

/// Parse a contract file string into a `ContractRegistry`.
///
/// Each `axiom` clause in the source becomes one registered contract under
/// `ArrSource::Native`. Returns an error on malformed input.
pub fn parse_contracts(source: &str) -> Result<ContractRegistry, String> {
    let mut reg = ContractRegistry::new();
    for axiom in AxiomIter::new(source) {
        let axiom = axiom?;
        let name = axiom.name.clone();
        let contract = axiom.to_contract()?;
        reg.register(name, ArrSource::Native, contract);
    }
    Ok(reg)
}

/// Load all `.hnl` contract files from a directory and merge them into a
/// single `ContractRegistry`.
pub fn load_dir(dir: &str) -> Result<ContractRegistry, String> {
    let mut reg = ContractRegistry::new();
    let path = Path::new(dir);
    if !path.is_dir() {
        return Err(format!("not a directory: {}", dir));
    }
    let mut entries: Vec<_> = std::fs::read_dir(path)
        .map_err(|e| format!("cannot read directory {}: {}", dir, e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("hnl"))
        .collect();
    entries.sort();
    for entry in entries {
        let source = std::fs::read_to_string(&entry)
            .map_err(|e| format!("cannot read {}: {}", entry.display(), e))?;
        let sub = parse_contracts(&source)?;
        reg.merge(sub);
    }
    Ok(reg)
}

/// Load the standard axiom set bundled with the crate at compile time.
///
/// This parses the bundled `prim.hnl` and `list.hnl` contract files (via
/// `include_str!`) and returns a `ContractRegistry` populated with the
/// canonical axioms. This is the new canonical way to obtain the minimal
/// registry; `ContractRegistry::minimal()` and `standard()` are retained for
/// backward compatibility.
pub fn load_standard_axioms() -> ContractRegistry {
    let mut reg = parse_contracts(PRIM_CONTRACTS).expect("bundled prim.hnl must parse");
    let list_reg = parse_contracts(LIST_CONTRACTS).expect("bundled list.hnl must parse");
    reg.merge(list_reg);
    let parse_reg = parse_contracts(PARSE_CONTRACTS).expect("bundled parse.hnl must parse");
    reg.merge(parse_reg);
    let state_reg = parse_contracts(STATE_CONTRACTS).expect("bundled state.hnl must parse");
    reg.merge(state_reg);
    reg
}

// ============================================================================
// Axiom model
// ============================================================================

/// A parsed (but not yet registered) axiom.
struct ParsedAxiom {
    name: String,
    input: RefTy,
    output: RefTy,
    requires: Option<Pred>,
    guarantee: Option<Pred>,
    termination: Option<String>,
}

impl ParsedAxiom {
    fn to_contract(self) -> Result<Contract, String> {
        let guarantee = self.guarantee.ok_or_else(|| {
            format!("axiom `{}` is missing a `guarantee:` clause", self.name)
        })?;
        Ok(Contract::conditional(
            self.input,
            self.output,
            self.requires.unwrap_or(Pred::True),
            guarantee,
            self.termination,
        ))
    }
}

// ============================================================================
// Line-based axiom iterator
// ============================================================================

/// Iterator over axioms in a contract source string.
///
/// Skips comments (`// ...`) and blank lines. An axiom begins with a line
/// starting with `axiom ` at column 0. Subsequent indented lines
/// (`requires:`, `guarantee:`, `termination:`) attach to the current axiom.
struct AxiomIter<'a> {
    lines: Vec<&'a str>,
    idx: usize,
}

impl<'a> AxiomIter<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            lines: source.lines().collect(),
            idx: 0,
        }
    }
}

impl<'a> Iterator for AxiomIter<'a> {
    type Item = Result<ParsedAxiom, String>;

    fn next(&mut self) -> Option<Self::Item> {
        // Skip comments and blank lines until we hit an `axiom` line.
        while self.idx < self.lines.len() {
            let raw = self.lines[self.idx];
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                self.idx += 1;
                continue;
            }
            if trimmed.starts_with("axiom ") {
                break;
            }
            // A non-indented, non-axiom line at top level is an error.
            return Some(Err(format!(
                "unexpected top-level line (expected `axiom`): {}",
                raw.trim()
            )));
        }
        if self.idx >= self.lines.len() {
            return None;
        }

        let header = self.lines[self.idx].trim();
        self.idx += 1;
        let parsed_header = match parse_axiom_header(header) {
            Ok(h) => h,
            Err(e) => return Some(Err(e)),
        };
        let mut axiom = ParsedAxiom {
            name: parsed_header.name,
            input: parsed_header.input,
            output: parsed_header.output,
            requires: None,
            guarantee: None,
            termination: None,
        };

        // Consume indented clause lines belonging to this axiom.
        while self.idx < self.lines.len() {
            let raw = self.lines[self.idx];
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                self.idx += 1;
                continue;
            }
            // A clause line must be indented (leading whitespace present).
            if raw.chars().next().map(|c| c.is_whitespace()).unwrap_or(false) {
                self.idx += 1;
                if let Err(e) = apply_clause(&mut axiom, trimmed) {
                    return Some(Err(e));
                }
            } else {
                // Next axiom or top-level construct -- stop here.
                break;
            }
        }

        Some(Ok(axiom))
    }
}

struct ParsedHeader {
    name: String,
    input: RefTy,
    output: RefTy,
}

/// Parse an `axiom <name> : <type> -> <type>` header line.
///
/// Accepts both ASCII `->` and Unicode `->` as the arrow separator.
fn parse_axiom_header(line: &str) -> Result<ParsedHeader, String> {
    let rest = line
        .strip_prefix("axiom ")
        .ok_or_else(|| format!("axiom header must start with `axiom `: {}", line))?
        .trim();

    // Split on the first `:` separating the name from the signature.
    let colon = rest
        .find(':')
        .ok_or_else(|| format!("axiom header missing `:`: {}", line))?;
    let name = rest[..colon].trim().to_string();
    if name.is_empty() {
        return Err(format!("axiom header missing name: {}", line));
    }
    let sig = rest[colon + 1..].trim();

    // Split on the arrow `->`.
    let (input_src, output_src) = if let Some(idx) = sig.find("->") {
        let input = sig[..idx].trim();
        let output = sig[idx + 2..].trim();
        (input, output)
    } else {
        return Err(format!("axiom header missing `->`: {}", line));
    };
    if input_src.is_empty() || output_src.is_empty() {
        return Err(format!("axiom header has empty type: {}", line));
    }
    let input = parse_type(input_src)?;
    let output = parse_type(output_src)?;
    Ok(ParsedHeader {
        name,
        input,
        output,
    })
}

/// Apply an indented clause (`requires:`, `guarantee:`, `termination:`) to an
/// in-progress axiom.
fn apply_clause(axiom: &mut ParsedAxiom, clause: &str) -> Result<(), String> {
    let (key, rest) = split_clause(clause)?;
    match key {
        "requires" => {
            let pred = parse_pred(rest.trim())?;
            axiom.requires = Some(pred);
        }
        "guarantee" => {
            let pred = parse_pred(rest.trim())?;
            axiom.guarantee = Some(pred);
        }
        "termination" => {
            let eff = rest.trim().to_string();
            if !matches!(
                eff.as_str(),
                "decrements_length" | "decrements_counter" | "constant" | "none"
            ) {
                return Err(format!("unknown termination effect: {}", eff));
            }
            axiom.termination = Some(eff);
        }
        other => return Err(format!("unknown clause `{}`", other)),
    }
    Ok(())
}

/// Split a clause line `key: value` into its key and value parts.
fn split_clause(clause: &str) -> Result<(&str, &str), String> {
    let colon = clause
        .find(':')
        .ok_or_else(|| format!("clause missing `:`: {}", clause))?;
    Ok((clause[..colon].trim(), clause[colon + 1..].trim()))
}

// ============================================================================
// Type parser (recursive descent)
// ============================================================================

/// A small cursor over the type/predicate source being parsed.
struct Parser<'a> {
    chars: Vec<char>,
    pos: usize,
    _src: &'a str,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            chars: src.chars().collect(),
            pos: 0,
            _src: src,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    /// Try to consume a literal string (character-by-character). Returns true
    /// if it matched and advanced.
    fn try_consume(&mut self, s: &str) -> bool {
        let s_chars: Vec<char> = s.chars().collect();
        if self.pos + s_chars.len() > self.chars.len() {
            return false;
        }
        for (i, c) in s_chars.iter().enumerate() {
            if self.chars[self.pos + i] != *c {
                return false;
            }
        }
        self.pos += s_chars.len();
        true
    }

    /// Consume a literal string or return an error.
    fn expect(&mut self, s: &str) -> Result<(), String> {
        if self.try_consume(s) {
            Ok(())
        } else {
            Err(format!("expected `{}` at: {}", s, self.tail()))
        }
    }

    fn tail(&self) -> String {
        self.chars[self.pos.min(self.chars.len())..]
            .iter()
            .collect()
    }
}

/// Parse a type expression into a `RefTy`.
pub fn parse_type(src: &str) -> Result<RefTy, String> {
    let mut p = Parser::new(src);
    p.skip_ws();
    let ty = parse_type_inner(&mut p)?;
    p.skip_ws();
    if !p.eof() {
        return Err(format!("trailing characters in type: {}", p.tail()));
    }
    Ok(ty)
}

fn parse_type_inner(p: &mut Parser) -> Result<RefTy, String> {
    p.skip_ws();
    match p.peek() {
        Some('(') => parse_paren_type(p),
        Some('{') => parse_refine_type(p),
        Some(c) if is_ident_start(c) => parse_name_type(p),
        Some(c) => Err(format!("unexpected char `{}` in type: {}", c, p.tail())),
        None => Err("unexpected end of type".to_string()),
    }
}

/// Parse a parenthesized type: either `(T * T)` (product) or `(T + T)` (sum).
///
/// Accepts both ASCII `*` and Unicode `*` as the product separator.
fn parse_paren_type(p: &mut Parser) -> Result<RefTy, String> {
    p.expect("(")?;
    p.skip_ws();
    let lhs = parse_type_inner(p)?;
    p.skip_ws();
    // Look for the separator: `*` (product) or `+` (sum).
    let sep = p.peek().ok_or("expected `*` or `+` in parenthesized type")?;
    let is_prod = sep == '*';
    let is_sum = sep == '+';
    if !is_prod && !is_sum {
        return Err(format!("expected `*` or `+` in parenthesized type: {}", p.tail()));
    }
    p.next();
    p.skip_ws();
    let rhs = parse_type_inner(p)?;
    p.skip_ws();
    p.expect(")")?;
    if is_prod {
        Ok(RefTy::Prod(Box::new(lhs), Box::new(rhs)))
    } else {
        Ok(RefTy::Sum(Box::new(lhs), Box::new(rhs)))
    }
}

/// Parse a refinement type `{ x : T | pred }`.
fn parse_refine_type(p: &mut Parser) -> Result<RefTy, String> {
    p.expect("{")?;
    p.skip_ws();
    // Bind variable name (e.g. `x`).
    let _var = parse_ident(p)?;
    p.skip_ws();
    p.expect(":")?;
    p.skip_ws();
    // The base type.
    let base = parse_base_ty(p)?;
    p.skip_ws();
    p.expect("|")?;
    p.skip_ws();
    let pred = parse_pred_inner(p)?;
    p.skip_ws();
    p.expect("}")?;
    Ok(RefTy::Refine(base, pred))
}

/// Parse a name-led type: `Int`, `Bool`, `a`, `List T`.
fn parse_name_type(p: &mut Parser) -> Result<RefTy, String> {
    let name = parse_ident(p)?;
    match name.as_str() {
        "Int" => Ok(RefTy::Plain(BaseTy::Int)),
        "Bool" => Ok(RefTy::Plain(BaseTy::Bool)),
        "List" => {
            p.skip_ws();
            let elem = parse_type_inner(p)?;
            Ok(RefTy::List(Box::new(elem)))
        }
        other => {
            // A single greek letter (or any identifier) is a type variable.
            Ok(RefTy::Plain(BaseTy::Var(other.to_string())))
        }
    }
}

/// Parse a base type (used inside `{ x : T | ... }`).
fn parse_base_ty(p: &mut Parser) -> Result<BaseTy, String> {
    p.skip_ws();
    let name = parse_ident(p)?;
    match name.as_str() {
        "Int" => Ok(BaseTy::Int),
        "Bool" => Ok(BaseTy::Bool),
        other => Ok(BaseTy::Var(other.to_string())),
    }
}

/// Parse an identifier: a run of ident-continuation characters. We allow
/// greek letters and alphanumerics; the first char must be an ident-start.
fn parse_ident(p: &mut Parser) -> Result<String, String> {
    p.skip_ws();
    let start = p.pos;
    match p.peek() {
        Some(c) if is_ident_start(c) => {
            p.next();
            while let Some(c) = p.peek() {
                if is_ident_cont(c) {
                    p.next();
                } else {
                    break;
                }
            }
            Ok(p.chars[start..p.pos].iter().collect())
        }
        Some(c) => Err(format!("expected identifier, got `{}`", c)),
        None => Err("expected identifier, got end of input".to_string()),
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_cont(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.'
}

// ============================================================================
// Predicate parser (recursive descent)
// ============================================================================

/// Parse a predicate expression into a `Pred`.
pub fn parse_pred(src: &str) -> Result<Pred, String> {
    let mut p = Parser::new(src);
    p.skip_ws();
    let pred = parse_pred_inner(&mut p)?;
    p.skip_ws();
    if !p.eof() {
        return Err(format!("trailing characters in predicate: {}", p.tail()));
    }
    Ok(pred)
}

fn parse_pred_inner(p: &mut Parser) -> Result<Pred, String> {
    parse_pred_implies(p)
}

/// `==>` is right-associative and lowest precedence.
fn parse_pred_implies(p: &mut Parser) -> Result<Pred, String> {
    let lhs = parse_pred_or(p)?;
    p.skip_ws();
    if p.try_consume("==>") {
        p.skip_ws();
        let rhs = parse_pred_implies(p)?;
        Ok(Pred::Implies(Box::new(lhs), Box::new(rhs)))
    } else {
        Ok(lhs)
    }
}

/// `||` is next.
fn parse_pred_or(p: &mut Parser) -> Result<Pred, String> {
    let mut lhs = parse_pred_and(p)?;
    loop {
        p.skip_ws();
        if p.try_consume("||") {
            p.skip_ws();
            let rhs = parse_pred_and(p)?;
            lhs = Pred::Or(Box::new(lhs), Box::new(rhs));
        } else {
            break;
        }
    }
    Ok(lhs)
}

/// `&&` is next.
fn parse_pred_and(p: &mut Parser) -> Result<Pred, String> {
    let mut lhs = parse_pred_atom(p)?;
    loop {
        p.skip_ws();
        if p.try_consume("&&") {
            p.skip_ws();
            let rhs = parse_pred_atom(p)?;
            lhs = Pred::And(Box::new(lhs), Box::new(rhs));
        } else {
            break;
        }
    }
    Ok(lhs)
}

/// Parse a predicate atom: `True`, `False`, a named predicate like
/// `Sorted(x)`, a parenthesized predicate, or a `forall` quantifier.
fn parse_pred_atom(p: &mut Parser) -> Result<Pred, String> {
    p.skip_ws();
    match p.peek() {
        Some('(') => {
            p.next();
            p.skip_ws();
            let inner = parse_pred_inner(p)?;
            p.skip_ws();
            p.expect(")")?;
            Ok(inner)
        }
        Some('{') => {
            // A refinement-type-as-predicate is unusual here; treat the
            // inner predicate as the atom.
            p.next();
            p.skip_ws();
            let _var = parse_ident(p)?;
            p.skip_ws();
            p.expect(":")?;
            p.skip_ws();
            let _base = parse_base_ty(p)?;
            p.skip_ws();
            p.expect("|")?;
            p.skip_ws();
            let pred = parse_pred_inner(p)?;
            p.skip_ws();
            p.expect("}")?;
            Ok(pred)
        }
        Some(c) if is_ident_start(c) => {
            let name = parse_ident(p)?;
            match name.as_str() {
                "True" => Ok(Pred::True),
                "False" => Ok(Pred::False),
                "forall" => parse_forall(p),
                other => parse_named_pred(p, other),
            }
        }
        Some(c) => Err(format!("unexpected char `{}` in predicate: {}", c, p.tail())),
        None => Err("unexpected end of predicate".to_string()),
    }
}

/// Parse `forall x in list, pred` (the `pred` is a single atom, typically a
/// named predicate application like `p(x)`).
fn parse_forall(p: &mut Parser) -> Result<Pred, String> {
    p.skip_ws();
    let var = parse_ident(p)?;
    p.skip_ws();
    p.expect("in")?;
    p.skip_ws();
    let list = parse_ident(p)?;
    p.skip_ws();
    // Optional comma separator.
    if p.peek() == Some(',') {
        p.next();
    }
    p.skip_ws();
    // The body predicate: a named application `name(arg)`.
    let pred_name = parse_ident(p)?;
    p.skip_ws();
    p.expect("(")?;
    p.skip_ws();
    let _arg = parse_ident(p)?;
    p.skip_ws();
    p.expect(")")?;
    Ok(Pred::Forall(var, list, pred_name))
}

/// Parse a named predicate application: `Sorted(x)`, `LengthEq(a, b)`, etc.
fn parse_named_pred(p: &mut Parser, name: &str) -> Result<Pred, String> {
    p.skip_ws();
    p.expect("(")?;
    let args = parse_pred_args(p)?;
    p.skip_ws();
    p.expect(")")?;
    match name {
        "Sorted" => {
            one_arg(name, &args)?;
            Ok(Pred::Sorted(args[0].clone()))
        }
        "LengthEq" => {
            two_args(name, &args)?;
            Ok(Pred::LengthEq(args[0].clone(), args[1].clone()))
        }
        "LengthLe" => {
            two_args(name, &args)?;
            Ok(Pred::LengthLe(args[0].clone(), args[1].clone()))
        }
        "AllSatisfy" => {
            two_args(name, &args)?;
            Ok(Pred::AllSatisfy(args[0].clone(), args[1].clone()))
        }
        "Le" => {
            two_args(name, &args)?;
            Ok(Pred::Le(args[0].clone(), args[1].clone()))
        }
        "HeadOf" => {
            two_args(name, &args)?;
            Ok(Pred::HeadOf(args[0].clone(), args[1].clone()))
        }
        "LengthPlusOne" => {
            two_args(name, &args)?;
            Ok(Pred::LengthPlusOne(args[0].clone(), args[1].clone()))
        }
        "LengthSubOne" => {
            two_args(name, &args)?;
            Ok(Pred::LengthSubOne(args[0].clone(), args[1].clone()))
        }
        "IsPerm" => {
            two_args(name, &args)?;
            Ok(Pred::IsPerm(args[0].clone(), args[1].clone()))
        }
        "IsSuccess" => {
            one_arg(name, &args)?;
            Ok(Pred::IsSuccess(args[0].clone()))
        }
        "IsError" => {
            one_arg(name, &args)?;
            Ok(Pred::IsError(args[0].clone()))
        }
        other => Err(format!("unknown predicate `{}`", other)),
    }
}

/// Parse comma-separated predicate arguments (identifiers or integer literals).
fn parse_pred_args(p: &mut Parser) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    p.skip_ws();
    if p.peek() == Some(')') {
        return Ok(args);
    }
    loop {
        p.skip_ws();
        let arg = parse_pred_arg(p)?;
        args.push(arg);
        p.skip_ws();
        if p.peek() == Some(',') {
            p.next();
            continue;
        }
        break;
    }
    Ok(args)
}

/// A predicate argument is either an identifier or an integer literal.
fn parse_pred_arg(p: &mut Parser) -> Result<String, String> {
    p.skip_ws();
    match p.peek() {
        Some(c) if is_ident_start(c) => parse_ident(p),
        Some(c) if c.is_ascii_digit() => {
            let mut s = String::new();
            while let Some(c) = p.peek() {
                if c.is_ascii_digit() {
                    s.push(c);
                    p.next();
                } else {
                    break;
                }
            }
            Ok(s)
        }
        Some(c) => Err(format!("unexpected char in predicate arg: `{}`", c)),
        None => Err("expected predicate arg, got end of input".to_string()),
    }
}

fn one_arg(name: &str, args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err(format!("`{}` expects 1 argument, got {}", name, args.len()));
    }
    Ok(())
}

fn two_args(name: &str, args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err(format!("`{}` expects 2 arguments, got {}", name, args.len()));
    }
    Ok(())
}
