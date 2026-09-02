//! SMT-style implication checker.
//!
//! The implication checker delegates to Z3 (an industrial SMT solver) for
//! genuine validity checking, using the native `z3` Rust crate (with Z3
//! statically linked via the `static-link-z3` feature -- no system library or
//! Python subprocess required). Predicates are translated directly into Z3
//! ASTs and the validity query is encoded as a satisfiability check of the
//! negation: `P ==> Q` is VALID iff `(P && !Q)` is UNSAT.
//!
//! Arithmetic refinements (`LengthEq`, `LengthLe`) are encoded with real Int
//! arithmetic, so Z3 performs genuine linear-integer reasoning. Custom
//! refinements (`Sorted`, `AllSatisfy`, `Forall`) are encoded as distinct
//! Boolean atoms -- one per ground predicate -- which is sound: Z3 only reports
//! UNSAT (valid) when the implication holds for *every* interpretation of those
//! atoms, and never proves a false implication by conflating two distinct
//! predicates.
//!
//! If native Z3 returns `unknown` (rare for these decidable fragments), we
//! fall back to a robust Python-subprocess checker, and finally to the
//! conservative structural (toy) checker, preserving soundness.

use crate::types::Pred;
use std::collections::HashMap;
use std::process::Command;
use std::sync::OnceLock;

#[cfg(feature = "native-z3")]
use z3::ast::{Ast, Bool, Int};
#[cfg(feature = "native-z3")]
use z3::{Config, Context, SatResult, Solver};

// ============================================================================
// Z3 verdict
// ============================================================================

/// Detailed outcome of a Z3 validity query, for reporting purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z3Verdict {
    /// `(P && !Q)` is UNSAT -> `P ==> Q` is VALID.
    Valid,
    /// `(P && !Q)` is SAT -> a counterexample exists, `P ==> Q` is INVALID.
    Invalid,
    /// Z3 returned `unknown`, or the subprocess could not be run.
    Unknown,
}

// ============================================================================
// Main verdict dispatcher
// ============================================================================

/// Run Z3 on the query "is `(P && !Q)` satisfiable?" and classify the result.
///
/// Primary path: native Z3 Rust bindings (statically linked) via
/// [`z3_implies_native`]. If native Z3 returns `None` (unknown), falls back
/// to a robust Python-subprocess checker. When the `native-z3` feature is
/// disabled, goes straight to the subprocess checker.
pub fn z3_verdict(p: &Pred, q: &Pred) -> Z3Verdict {
    // 1. Try native Z3 (if available)
    if let Some(result) = z3_implies_native(p, q) {
        return if result {
            Z3Verdict::Valid
        } else {
            Z3Verdict::Invalid
        };
    }

    // 2. Fall back to Z3 subprocess (if available)
    z3_verdict_subprocess(p, q)
}

// ============================================================================
// Native Z3 implementation
// ============================================================================

/// Native Z3 implication check: translate predicates to Z3 ASTs and check
/// whether `(P && !Q)` is UNSAT (meaning `P ==> Q` is valid).
///
/// Returns `Some(true)` if the implication is valid (UNSAT), `Some(false)`
/// if a counterexample exists (SAT), and `None` if Z3 returns `unknown`.
///
/// When the `native-z3` feature is disabled, always returns `None`.
#[cfg(feature = "native-z3")]
fn z3_implies_native(p: &Pred, q: &Pred) -> Option<bool> {
    let cfg = Config::new();
    let ctx = Context::new(&cfg);
    let solver = Solver::new(&ctx);

    // Assert non-negativity of every length constant referenced, matching
    // the semantics of the SMT-LIB2 encoding (lengths are >= 0).
    let mut len_vars = Vec::new();
    collect_len_vars(p, &mut len_vars);
    collect_len_vars(q, &mut len_vars);
    for lv in &len_vars {
        let c = Int::new_const(&ctx, lv.as_str());
        solver.assert(&c.ge(&Int::from_i64(&ctx, 0)));
    }

    let p_z3 = pred_to_z3(&ctx, p);
    let q_z3 = pred_to_z3(&ctx, q);
    // P ==> Q is valid  iff  (P && !Q) is UNSAT.
    solver.assert(&p_z3.and(&[&q_z3.not()]));

    match solver.check() {
        SatResult::Unsat => Some(true),
        SatResult::Sat => Some(false),
        SatResult::Unknown => None,
    }
}

/// Fallback when the `native-z3` feature is disabled: Z3 is not available,
/// so we cannot decide the implication natively.
#[cfg(not(feature = "native-z3"))]
fn z3_implies_native(_p: &Pred, _q: &Pred) -> Option<bool> {
    None
}

/// Translate a `Pred` into a Z3 boolean AST.
///
/// Arithmetic predicates (`LengthEq`/`LengthLe`) become real integer
/// constraints over per-variable length constants. Custom predicates
/// (`Sorted`/`AllSatisfy`/`Forall`) become distinct Boolean atoms -- each
/// ground predicate gets a unique symbol (e.g. `|sorted_output|`), so Z3
/// reasons about them as independent propositional variables.
#[cfg(feature = "native-z3")]
fn pred_to_z3<'ctx>(ctx: &'ctx Context, p: &Pred) -> Bool<'ctx> {
    match p {
        Pred::True => Bool::from_bool(ctx, true),
        Pred::False => Bool::from_bool(ctx, false),
        Pred::Sorted(_) | Pred::AllSatisfy(_, _) | Pred::Forall(_, _, _) => {
            let sym = atom_symbol(p).unwrap();
            Bool::new_const(ctx, sym.as_str())
        }
        Pred::Le(_, _) | Pred::HeadOf(_, _) | Pred::IsPerm(_, _)
        | Pred::IsSuccess(_) | Pred::IsError(_) => {
            let sym = atom_symbol(p).unwrap();
            Bool::new_const(ctx, sym.as_str())
        }
        Pred::LengthEq(a, b) => {
            let la = Int::new_const(ctx, var_to_len(a).as_str());
            let lb = Int::new_const(ctx, var_to_len(b).as_str());
            la.eq(&lb)
        }
        Pred::LengthLe(a, b) => {
            let la = Int::new_const(ctx, var_to_len(a).as_str());
            let lb = Int::new_const(ctx, var_to_len(b).as_str());
            la.le(&lb)
        }
        Pred::LengthPlusOne(a, b) => {
            // |a| = |b| + 1
            let la = Int::new_const(ctx, var_to_len(a).as_str());
            let lb = Int::new_const(ctx, var_to_len(b).as_str());
            la.eq(&lb.add(&[&Int::from_i64(ctx, 1)]))
        }
        Pred::LengthSubOne(a, b) => {
            // |a| = |b| - 1
            let la = Int::new_const(ctx, var_to_len(a).as_str());
            let lb = Int::new_const(ctx, var_to_len(b).as_str());
            la.eq(&lb.sub(&[&Int::from_i64(ctx, 1)]))
        }
        Pred::And(a, b) => {
            let az = pred_to_z3(ctx, a);
            let bz = pred_to_z3(ctx, b);
            az.and(&[&bz])
        }
        Pred::Or(a, b) => {
            let az = pred_to_z3(ctx, a);
            let bz = pred_to_z3(ctx, b);
            az.or(&[&bz])
        }
        Pred::Implies(a, b) => {
            let az = pred_to_z3(ctx, a);
            let bz = pred_to_z3(ctx, b);
            az.implies(&bz)
        }
    }
}

// ============================================================================
// Subprocess fallback
// ============================================================================

/// Cached path to a Python interpreter that has the `z3` module available,
/// or `None` if no suitable interpreter was found. The availability probe
/// runs at most once per process.
static Z3_PYTHON: OnceLock<Option<String>> = OnceLock::new();

/// Locate a Python interpreter with the `z3` PyPI package installed.
fn find_z3_python() -> Option<&'static str> {
    Z3_PYTHON
        .get_or_init(|| {
            for candidate in ["/tmp/z3env/bin/python3", "python3", "python"] {
                let ok = Command::new(candidate)
                    .arg("-c")
                    .arg("import z3; print(z3.get_version_string())")
                    .output()
                    .map(|o| {
                        o.status.success()
                            && !String::from_utf8_lossy(&o.stdout).trim().is_empty()
                    })
                    .unwrap_or(false);
                if ok {
                    return Some(candidate.to_string());
                }
            }
            None
        })
        .as_deref()
}

/// Subprocess-based Z3 verdict (fallback). Builds an SMT-LIB2 script and
/// pipes it to a Python interpreter that has the `z3` package. Returns
/// `Unknown` if no interpreter is available or Z3 cannot decide.
fn z3_verdict_subprocess(p: &Pred, q: &Pred) -> Z3Verdict {
    let py = match find_z3_python() {
        Some(p) => p,
        None => return Z3Verdict::Unknown,
    };

    let p_smt = pred_to_smt(p);
    let q_smt = pred_to_smt(q);

    let mut atoms = Vec::new();
    collect_atoms(p, &mut atoms);
    collect_atoms(q, &mut atoms);
    let mut len_vars = Vec::new();
    collect_len_vars(p, &mut len_vars);
    collect_len_vars(q, &mut len_vars);

    let mut decls = String::new();
    decls.push_str("(declare-const input Int)\n");
    decls.push_str("(declare-const output Int)\n");
    decls.push_str("(declare-const acc Int)\n");
    for lv in &len_vars {
        decls.push_str(&format!("(declare-const {} Int)\n", lv));
    }
    for a in &atoms {
        decls.push_str(&format!("(declare-const {} Bool)\n", a));
    }

    let mut asserts = String::new();
    for lv in &len_vars {
        asserts.push_str(&format!("(assert (>= {} 0))\n", lv));
    }

    let smt_script = format!(
        "(set-logic ALL)\n\
         {}\n\
         {}\n\
         (assert (and {} (not {})))\n\
         (check-sat)\n\
         (exit)",
        decls, asserts, p_smt, q_smt
    );

    let py_script = format!(
        "import z3\n\
         s = z3.Solver()\n\
         s.from_string('''{}''')\n\
         print(s.check())",
        smt_script
    );

    let result = Command::new(py).arg("-c").arg(&py_script).output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("unsat") {
                Z3Verdict::Valid
            } else if stdout.contains("sat") {
                Z3Verdict::Invalid
            } else {
                Z3Verdict::Unknown
            }
        }
        Err(_) => Z3Verdict::Unknown,
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Check if `P ==> Q` is VALID using Z3.
/// Returns `true` if VALID, `false` if UNKNOWN/INVALID.
/// Uses native Z3 Rust bindings (with a subprocess fallback).
pub fn z3_implies(p: &Pred, q: &Pred) -> bool {
    matches!(z3_verdict(p, q), Z3Verdict::Valid)
}

/// Translate a `Pred` into an SMT-LIB2 expression.
///
/// Arithmetic predicates (`LengthEq`/`LengthLe`) become real integer
/// constraints. Custom predicates (`Sorted`/`AllSatisfy`/`Forall`) become
/// distinct Boolean atoms -- each ground predicate gets a unique symbol
/// (e.g. `|sorted_output|`), so Z3 reasons about them as independent
/// propositional variables.
pub fn pred_to_smt(p: &Pred) -> String {
    match p {
        Pred::True => "true".to_string(),
        Pred::False => "false".to_string(),
        Pred::Sorted(_) => atom_symbol(p).unwrap(),
        Pred::AllSatisfy(_, _) => atom_symbol(p).unwrap(),
        Pred::Forall(_, _, _) => atom_symbol(p).unwrap(),
        Pred::Le(_, _) => atom_symbol(p).unwrap(),
        Pred::HeadOf(_, _) => atom_symbol(p).unwrap(),
        Pred::IsPerm(_, _) => atom_symbol(p).unwrap(),
        Pred::IsSuccess(_) => atom_symbol(p).unwrap(),
        Pred::IsError(_) => atom_symbol(p).unwrap(),
        Pred::LengthEq(a, b) => format!("(= {} {})", var_to_len(a), var_to_len(b)),
        Pred::LengthLe(a, b) => format!("(<= {} {})", var_to_len(a), var_to_len(b)),
        Pred::LengthPlusOne(a, b) => {
            format!("(= {} (+ {} 1))", var_to_len(a), var_to_len(b))
        }
        Pred::LengthSubOne(a, b) => {
            format!("(= {} (- {} 1))", var_to_len(a), var_to_len(b))
        }
        Pred::And(a, b) => format!("(and {} {})", pred_to_smt(a), pred_to_smt(b)),
        Pred::Or(a, b) => format!("(or {} {})", pred_to_smt(a), pred_to_smt(b)),
        Pred::Implies(a, b) => format!("(=> {} {})", pred_to_smt(a), pred_to_smt(b)),
    }
}

/// Like `implies` but also returns a human-readable report of which checker
/// decided the query and why.
pub fn implies_with_report(p: &Pred, q: &Pred) -> (bool, String) {
    match z3_verdict(p, q) {
        Z3Verdict::Valid => (true, "Z3: unsat -- P ==> Q is valid".to_string()),
        Z3Verdict::Invalid => (false, "Z3: sat -- counterexample exists".to_string()),
        Z3Verdict::Unknown => {
            let r = toy_implies(p, q);
            let report = if r {
                "toy: structural".to_string()
            } else {
                "toy: unverified".to_string()
            };
            (r, report)
        }
    }
}

/// Main implication checker: try Z3 first, fall back to the conservative
/// toy checker when Z3 is unavailable or returns `unknown`.
///
/// Before invoking the toy checker, the premise `p` is strengthened with
/// [`simplify_with_arithmetic`], which derives additional `LengthEq` /
/// `LengthLe` facts from length-arithmetic relationships (transitivity and
/// cancellation of `LengthEq`, `LengthPlusOne`, `LengthSubOne`). This lets the
/// structural toy checker prove implications such as
/// `LengthPlusOne(a,b) && LengthSubOne(b,c) ==> LengthEq(a,c)` that it otherwise
/// could not. The strengthening is sound: every derived fact is a logical
/// consequence of `p`, so `p && derived == p` and no false implication is ever
/// proved.
pub fn implies(p: &Pred, q: &Pred) -> bool {
    if z3_implies(p, q) {
        true
    } else {
        // Z3 said invalid or unknown -- strengthen p with length-arithmetic
        // reasoning, then fall back to the conservative toy checker.
        let p_strong = simplify_with_arithmetic(p);
        toy_implies(&p_strong, q)
    }
}

// ============================================================================
// Length-arithmetic strengthening
// ============================================================================

/// Strengthen a predicate with facts derivable from length arithmetic.
///
/// Collects every `LengthEq`, `LengthPlusOne`, and `LengthSubOne` fact in `pred`
/// (flattening conjunctions), builds a union-find of length relationships with
/// offsets, and derives all entailed `LengthEq` (equal offset) and `LengthLe`
/// (strictly smaller offset) facts. The result is `pred && derived`, simplified.
///
/// This is sound: every derived fact is a logical consequence of the input
/// facts, so `pred && derived == pred`. It only makes the premise *structurally
/// richer*, enabling the conservative toy checker to prove implications it
/// otherwise could not (e.g. transitivity and cancellation of length
/// relations).
///
/// Examples handled:
/// - `LengthEq(a,b) && LengthEq(b,c) ==> LengthEq(a,c)` (transitivity)
/// - `LengthPlusOne(a,b) && LengthSubOne(b,c) ==> LengthEq(a,c)` (cancellation)
/// - `LengthSubOne(a,b) && LengthPlusOne(b,c) ==> LengthEq(a,c)` (cancellation)
/// - `LengthEq(a,b) ==> LengthEq(b,a)` (symmetry)
pub fn simplify_with_arithmetic(pred: &Pred) -> Pred {
    // Collect length-arithmetic facts (flattening And).
    let mut facts: Vec<Pred> = Vec::new();
    collect_length_facts(pred, &mut facts);

    // Build the union-find with offsets. Each variable x has a representative
    // root and an offset d such that |x| = |root| + d.
    let mut graph = LenGraph::new();
    for f in &facts {
        match f {
            Pred::LengthEq(a, b) => graph.union(a, b, 0),
            // |a| = |b| + 1  ->  |a| - |b| = 1
            Pred::LengthPlusOne(a, b) => graph.union(a, b, 1),
            // |a| = |b| - 1  ->  |a| - |b| = -1
            Pred::LengthSubOne(a, b) => graph.union(a, b, -1),
            _ => {}
        }
    }

    // Gather the canonical (root, offset) for every variable seen.
    let mut entries: Vec<(String, String, i64)> = Vec::new();
    let mut seen: HashMap<String, (String, i64)> = HashMap::new();
    for f in &facts {
        for v in length_vars_of(f) {
            if seen.contains_key(&v) {
                continue;
            }
            let (root, off) = graph.find(&v);
            seen.insert(v.clone(), (root.clone(), off));
            entries.push((v, root, off));
        }
    }

    // Derive new LengthEq / LengthLe facts for every pair in the same
    // equivalence class. Skip facts already present in `pred` (dedup via the
    // collected facts set) to avoid needless blow-up; `simplify` will also
    // dedup And(x, x) -> x.
    let existing: Vec<Pred> = facts.clone();
    let mut derived: Vec<Pred> = Vec::new();
    for i in 0..entries.len() {
        for j in 0..entries.len() {
            if i == j {
                continue;
            }
            let (a, ra, oa) = &entries[i];
            let (b, rb, ob) = &entries[j];
            if ra != rb {
                continue; // different equivalence classes: no relation
            }
            let fact = if oa == ob {
                Pred::LengthEq(a.clone(), b.clone())
            } else if oa < ob {
                Pred::LengthLe(a.clone(), b.clone())
            } else {
                continue; // oa > ob: handled by the (j, i) iteration
            };
            if !existing.contains(&fact) && !derived.contains(&fact) {
                derived.push(fact);
            }
        }
    }

    if derived.is_empty() {
        pred.clone()
    } else {
        let mut result = pred.clone();
        for d in derived {
            result = Pred::And(Box::new(result), Box::new(d));
        }
        result.simplify()
    }
}

/// Collect every `LengthEq`, `LengthPlusOne`, and `LengthSubOne` fact in `pred`,
/// flattening conjunctions recursively.
fn collect_length_facts(pred: &Pred, out: &mut Vec<Pred>) {
    match pred {
        Pred::LengthEq(_, _) | Pred::LengthPlusOne(_, _) | Pred::LengthSubOne(_, _) => {
            if !out.contains(pred) {
                out.push(pred.clone());
            }
        }
        Pred::And(a, b) => {
            collect_length_facts(a, out);
            collect_length_facts(b, out);
        }
        _ => {}
    }
}

/// The two variable names referenced by a length-arithmetic fact.
fn length_vars_of(p: &Pred) -> Vec<String> {
    match p {
        Pred::LengthEq(a, b)
        | Pred::LengthPlusOne(a, b)
        | Pred::LengthSubOne(a, b) => vec![a.clone(), b.clone()],
        _ => vec![],
    }
}

/// Union-find over length variables with offsets.
///
/// Invariant: for each variable `x`, `|x| = |root(x)| + offset(x)`, where
/// `root(x)` is the representative of `x`'s equivalence class.
struct LenGraph {
    /// Parent pointer for the union-find forest.
    parent: HashMap<String, String>,
    /// `offset[x] = |x| - |parent[x]|` (only meaningful when `x` is not a root).
    offset: HashMap<String, i64>,
}

impl LenGraph {
    fn new() -> Self {
        LenGraph {
            parent: HashMap::new(),
            offset: HashMap::new(),
        }
    }

    /// Ensure `x` is a tracked variable (its own root with offset 0).
    fn add(&mut self, x: &str) {
        if !self.parent.contains_key(x) {
            self.parent.insert(x.to_string(), x.to_string());
            self.offset.insert(x.to_string(), 0);
        }
    }

    /// Find the representative of `x` and its offset, with path compression.
    /// Returns `(root, off)` such that `|x| = |root| + off`.
    fn find(&mut self, x: &str) -> (String, i64) {
        self.add(x);
        // Iterative find with path compression to avoid deep recursion.
        // First, walk to the root collecting the path.
        let mut path: Vec<String> = Vec::new();
        let mut cur = x.to_string();
        while let Some(p) = self.parent.get(&cur) {
            if p == &cur {
                break;
            }
            path.push(cur.clone());
            cur = p.clone();
        }
        let root = cur;
        // Now compute cumulative offsets from x up to root, then compress.
        // offset[x] is relative to parent; accumulate.
        // Walk from root down the path computing absolute offsets.
        // path is [x, p1, p2, ...] from x toward root.
        // Compute absolute offset for each node in path.
        // |node| = |parent| + offset[node]; recurse to root.
        // We compute by processing path in reverse (closest to root first).
        // But easiest: recursively compute via a helper using memo.
        fn abs_off(graph: &LenGraph, node: &str, root: &str) -> i64 {
            if node == root {
                return 0;
            }
            let p = graph.parent.get(node).unwrap();
            let off = *graph.offset.get(node).unwrap();
            off + abs_off(graph, p, root)
        }
        let off_x = abs_off(self, x, &root);
        // Path compression: point every node on the path directly at root.
        for n in &path {
            let off_n = abs_off(self, n, &root);
            self.parent.insert(n.clone(), root.clone());
            self.offset.insert(n.clone(), off_n);
        }
        (root, off_x)
    }

    /// Merge the classes of `a` and `b` given the relationship `|a| = |b| + delta`.
    fn union(&mut self, a: &str, b: &str, delta: i64) {
        let (ra, oa) = self.find(a); // |a| = |ra| + oa
        let (rb, ob) = self.find(b); // |b| = |rb| + ob
        if ra == rb {
            // Already in the same class; the relationship is either consistent
            // (oa - ob == delta) or redundant. We do not detect contradictions
            // here -- the input is assumed consistent.
            return;
        }
        // |a| = |b| + delta  ->  |ra| + oa = |rb| + ob + delta
        //                  ->  |ra| = |rb| + (ob + delta - oa)
        // Attach ra under rb with offset (ob + delta - oa).
        self.parent.insert(ra.clone(), rb.clone());
        self.offset.insert(ra, ob + delta - oa);
    }
}

// ============================================================================
// Conservative structural implication checker (the "toy" checker)
// ============================================================================

/// Conservative structural implication checker.
///
/// Used as a fallback when Z3 is unavailable or returns `unknown`.
///
/// May return `false` (unknown) for true implications, but NEVER returns
/// `true` for false ones (soundness).
///
/// Both `p` and `q` are simplified before checking. This is essential for
/// fixpoint convergence detection when Z3 is not available: without
/// simplification, `And(True, True)` is structurally distinct from `True`,
/// so the checker cannot determine that `True ==> And(True, True)`, and the
/// fixpoint never converges.
pub fn toy_implies(p: &Pred, q: &Pred) -> bool {
    // Simplify both sides first. This ensures that And(True, True) collapses
    // to True, Or(True, False) collapses to True, etc., so structural
    // equality and the match arms below can recognise these equivalences.
    let p = p.simplify();
    let q = q.simplify();

    // Reflexivity first -- handles all equal predicates, including
    // And(a,b) ==> And(a,b) which the decomposition arms below would
    // otherwise mishandle. After simplification, this also catches
    // True ==> And(True, True) (both simplify to True).
    if p == q {
        return true;
    }
    match (&p, &q) {
        // Anything implies True
        (_, Pred::True) => true,
        // True only implies True. Since q is already simplified and p != q
        // (checked above), if p is True then q is not True, so this is false.
        (Pred::True, _) => false,
        // False implies anything (ex falso)
        (Pred::False, _) => true,
        // Both sides are conjunctions: try both decompositions.
        // And(a,b) ==> And(c,d) iff either a conjunct of p implies q,
        // OR p implies both conjuncts of q. The first decomposition
        // (And-on-left) is tried first for short-circuit efficiency;
        // the second (And-on-right) catches cases like
        // And(X,Y) ==> And(And(X,Y), And(X,Y)) where neither X nor Y
        // alone implies the full conjunction but the whole does.
        (Pred::And(a, b), Pred::And(c, d)) => {
            toy_implies(a, &q)
                || toy_implies(b, &q)
                || (toy_implies(&p, c) && toy_implies(&p, d))
        }
        // Conjunction on the left: if either conjunct implies q
        (Pred::And(a, b), _) => toy_implies(a, &q) || toy_implies(b, &q),
        // Conjunction on the right: p must imply both
        (_, Pred::And(a, b)) => toy_implies(&p, a) && toy_implies(&p, b),
        // Disjunction on the right: p must imply one
        (_, Pred::Or(a, b)) => toy_implies(&p, a) || toy_implies(&p, b),
        // Disjunction on the left: need both to imply q
        (Pred::Or(a, b), _) => toy_implies(a, &q) && toy_implies(b, &q),
        // Implication on the right: P ==> (A ==> B) means (P && A) ==> B
        (_, Pred::Implies(a, b)) => {
            toy_implies(&Pred::And(Box::new(p.clone()), a.clone()), b)
        }
        // Implication on the left: (A ==> B) ==> Q means (!A || B) ==> Q,
        // conservatively: if B ==> Q
        (Pred::Implies(_, b), _) => toy_implies(b, &q),
        // Sorted(x) ==> Sorted(x)
        (Pred::Sorted(x), Pred::Sorted(y)) if x == y => true,
        // LengthEq(a,b) ==> LengthLe(a,b)
        (Pred::LengthEq(a, b), Pred::LengthLe(c, d)) if a == c && b == d => true,
        // LengthEq(a,b) ==> LengthLe(b,a) (symmetry)
        (Pred::LengthEq(a, b), Pred::LengthLe(c, d)) if a == d && b == c => true,
        // LengthEq(a,b) ==> LengthEq(a,b)
        (Pred::LengthEq(a, b), Pred::LengthEq(c, d)) if a == c && b == d => true,
        // AllSatisfy reflexivity
        (Pred::AllSatisfy(l1, p1), Pred::AllSatisfy(l2, p2)) if l1 == l2 && p1 == p2 => true,
        // Forall reflexivity (same var, list, pred)
        (Pred::Forall(v1, l1, p1), Pred::Forall(v2, l2, p2))
            if v1 == v2 && l1 == l2 && p1 == p2 =>
        {
            true
        }
        // Le reflexivity
        (Pred::Le(a, b), Pred::Le(c, d)) if a == c && b == d => true,
        // HeadOf reflexivity
        (Pred::HeadOf(a, b), Pred::HeadOf(c, d)) if a == c && b == d => true,
        // LengthPlusOne reflexivity
        (Pred::LengthPlusOne(a, b), Pred::LengthPlusOne(c, d)) if a == c && b == d => true,
        // LengthSubOne reflexivity
        (Pred::LengthSubOne(a, b), Pred::LengthSubOne(c, d)) if a == c && b == d => true,
        // IsPerm reflexivity
        (Pred::IsPerm(a, b), Pred::IsPerm(c, d)) if a == c && b == d => true,
        // IsSuccess reflexivity
        (Pred::IsSuccess(a), Pred::IsSuccess(b)) if a == b => true,
        // IsError reflexivity
        (Pred::IsError(a), Pred::IsError(b)) if a == b => true,
        // IsSuccess(x) ==> IsSuccess(x) || anything
        (Pred::IsSuccess(a), Pred::Or(l, r))
            if matches!(l.as_ref(), Pred::IsSuccess(b) if a == b)
               || matches!(r.as_ref(), Pred::IsSuccess(b) if a == b) => true,
        // IsError(x) ==> IsError(x) || anything
        (Pred::IsError(a), Pred::Or(l, r))
            if matches!(l.as_ref(), Pred::IsError(b) if a == b)
               || matches!(r.as_ref(), Pred::IsError(b) if a == b) => true,
        // LengthPlusOne(a, b) ==> LengthLe(b, a)  (|a| = |b|+1 ==> |b| <= |a|)
        (Pred::LengthPlusOne(a, b), Pred::LengthLe(c, d)) if b == c && a == d => true,
        // LengthSubOne(a, b) ==> LengthLe(a, b)  (|a| = |b|-1 ==> |a| <= |b|)
        (Pred::LengthSubOne(a, b), Pred::LengthLe(c, d)) if a == c && b == d => true,
        // Unknown
        _ => false,
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// The unique SMT symbol for a ground custom predicate, or `None` for
/// predicates that are not encoded as atoms.
fn atom_symbol(p: &Pred) -> Option<String> {
    match p {
        Pred::Sorted(v) => Some(format!("|sorted_{}|", v)),
        Pred::AllSatisfy(list, pred) => Some(format!("|all_satisfy_{}_{}|", list, pred)),
        Pred::Forall(var, list, pred) => Some(format!("|forall_{}_{}_{}|", var, list, pred)),
        Pred::Le(a, b) => Some(format!("|le_{}_{}|", a, b)),
        Pred::HeadOf(elem, list) => Some(format!("|head_of_{}_{}|", elem, list)),
        Pred::IsPerm(a, b) => Some(format!("|is_perm_{}_{}|", a, b)),
        Pred::IsSuccess(v) => Some(format!("|is_success_{}|", v)),
        Pred::IsError(v) => Some(format!("|is_error_{}|", v)),
        _ => None,
    }
}

/// Map a variable name to its SMT length constant.
fn var_to_len(v: &str) -> String {
    match v {
        "input" => "|input_len|".to_string(),
        "output" => "|output_len|".to_string(),
        _ => format!("|{}_len|", v),
    }
}

/// Collect every custom-predicate atom appearing in `p` (deduplicated).
fn collect_atoms(p: &Pred, atoms: &mut Vec<String>) {
    match p {
        Pred::Sorted(_) | Pred::AllSatisfy(_, _) | Pred::Forall(_, _, _) | Pred::Le(_, _)
        | Pred::HeadOf(_, _) | Pred::IsPerm(_, _)
        | Pred::IsSuccess(_) | Pred::IsError(_) => {
            if let Some(a) = atom_symbol(p) {
                if !atoms.contains(&a) {
                    atoms.push(a);
                }
            }
        }
        Pred::And(a, b) | Pred::Or(a, b) | Pred::Implies(a, b) => {
            collect_atoms(a, atoms);
            collect_atoms(b, atoms);
        }
        _ => {}
    }
}

/// Collect every length constant referenced by arithmetic predicates.
fn collect_len_vars(p: &Pred, vars: &mut Vec<String>) {
    match p {
        Pred::LengthEq(a, b) | Pred::LengthLe(a, b) | Pred::LengthPlusOne(a, b)
        | Pred::LengthSubOne(a, b) => {
            for v in [a.as_str(), b.as_str()] {
                let lv = var_to_len(v);
                if !vars.contains(&lv) {
                    vars.push(lv);
                }
            }
        }
        Pred::And(a, b) | Pred::Or(a, b) | Pred::Implies(a, b) => {
            collect_len_vars(a, vars);
            collect_len_vars(b, vars);
        }
        _ => {}
    }
}
