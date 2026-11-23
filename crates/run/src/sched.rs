//! `dependsOn` resolution — topo sort over a package's script table.
//!
//! `hnl run <name>` runs the named script *and its dependencies*;
//! [`resolve_order`] returns the execution order (dependencies first,
//! declaration order preserved among independents). Cycles and unknown
//! deps are diagnostics computed up front, before anything runs (P-003).

use std::collections::{HashMap, HashSet};

use crate::error::RunError;
use crate::pkg::PkgScripts;

/// Resolve `name` plus its transitive `dependsOn` closure into execution
/// order — dependencies before dependents, deterministic.
///
/// # Errors
///
/// - [`RunError::UnknownScript`] when `name` is undeclared (the available
///   names ride along for the npm-style hint);
/// - [`RunError::UnknownDep`] when a task's `dependsOn` names a script
///   that does not exist — the error names the task at fault;
/// - [`RunError::Cycle`] listing the cycle when `dependsOn` loops.
pub fn resolve_order(pkg: &PkgScripts, name: &str) -> Result<Vec<String>, RunError> {
    if pkg.get(name).is_none() {
        return Err(RunError::UnknownScript {
            name: name.to_string(),
            available: pkg.names().iter().map(|s| s.to_string()).collect(),
        });
    }

    // Iterative post-order DFS. `visiting` is the recursion stack (for
    // cycle paths), `done` the finished set; `path` mirrors `visiting`
    // for cycle reconstruction. Frames keep the DFS iterative so a
    // pathological dependsOn chain cannot overflow the stack.
    enum Frame<'a> {
        /// Enter a script: validate, mark visiting, schedule deps + exit.
        Enter(&'a str),
        /// All of a script's deps were scheduled; emit it.
        Exit(&'a str),
    }

    let mut visiting: HashSet<&str> = HashSet::new();
    let mut path: Vec<&str> = Vec::new();
    let mut done: HashSet<&str> = HashSet::new();
    let mut order: Vec<String> = Vec::new();
    let mut work: Vec<Frame> = vec![Frame::Enter(name)];

    while let Some(frame) = work.pop() {
        match frame {
            Frame::Enter(n) => {
                if done.contains(n) {
                    continue;
                }
                if visiting.contains(n) {
                    let pos = path.iter().position(|s| *s == n).unwrap_or(0);
                    let mut cycle: Vec<String> =
                        path[pos..].iter().map(|s| s.to_string()).collect();
                    cycle.push(n.to_string());
                    return Err(RunError::Cycle(cycle));
                }
                let script = pkg.get(n).expect("deps validated at expansion");
                visiting.insert(n);
                path.push(n);
                work.push(Frame::Exit(n));
                let deps: Vec<&str> = script
                    .task()
                    .map(|t| t.depends_on.iter().map(String::as_str).collect())
                    .unwrap_or_default();
                // Validate every dep up front (names the task at fault)
                // and schedule in reverse so they pop in declaration
                // order.
                for dep in deps.iter().rev() {
                    if pkg.get(dep).is_none() {
                        return Err(RunError::UnknownDep {
                            task: n.to_string(),
                            dep: dep.to_string(),
                        });
                    }
                    work.push(Frame::Enter(dep));
                }
            }
            Frame::Exit(n) => {
                visiting.remove(n);
                path.pop();
                done.insert(n);
                order.push(n.to_string());
            }
        }
    }
    Ok(order)
}

/// Topological sort of *all* scripts — the `hnl run` (no args) listing
/// order and a convenience for callers that want the whole DAG. Same
/// diagnostics as [`resolve_order`].
pub fn resolve_all(pkg: &PkgScripts) -> Result<Vec<String>, RunError> {
    let mut order = Vec::new();
    for name in pkg.names() {
        for n in resolve_order(pkg, name)? {
            if !order.contains(&n) {
                order.push(n);
            }
        }
    }
    Ok(order)
}

/// Cycle-check without ordering — exposed for `--plan`-style callers
/// that only want the diagnostic. Thin wrapper over
/// [`resolve_order`]'s error path.
pub fn check_acyclic(pkg: &PkgScripts, name: &str) -> Result<(), RunError> {
    resolve_order(pkg, name).map(|_| ())
}

/// Map name → dependsOn names, for callers rendering the DAG.
pub fn dependency_map(pkg: &PkgScripts) -> HashMap<String, Vec<String>> {
    pkg.scripts
        .iter()
        .map(|(name, script)| {
            (
                name.clone(),
                script
                    .task()
                    .map(|t| t.depends_on.clone())
                    .unwrap_or_default(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(src: &str) -> PkgScripts {
        PkgScripts::from_source(std::path::PathBuf::from("/nonexistent"), src.to_string())
    }

    #[test]
    fn topo_orders_dependencies_first() {
        let p = pkg(r#"{ scripts: {
            a: { run: "echo a", dependsOn: ["b"] },
            b: { run: "echo b", dependsOn: ["c"] },
            c: "echo c",
            d: "echo d"
        } }"#);
        assert_eq!(resolve_order(&p, "a").unwrap(), vec!["c", "b", "a"]);
        // Independent scripts are not pulled in.
        assert_eq!(resolve_order(&p, "d").unwrap(), vec!["d"]);
    }

    #[test]
    fn topo_diamond_dedups() {
        let p = pkg(r#"{ scripts: {
            top: { run: "echo t", dependsOn: ["l", "r"] },
            l: { run: "echo l", dependsOn: ["base"] },
            r: { run: "echo r", dependsOn: ["base"] },
            base: "echo base"
        } }"#);
        let order = resolve_order(&p, "top").unwrap();
        assert_eq!(order, vec!["base", "l", "r", "top"]);
    }

    #[test]
    fn cycle_is_a_diagnostic() {
        let p = pkg(r#"{ scripts: {
            a: { run: "echo a", dependsOn: ["b"] },
            b: { run: "echo b", dependsOn: ["a"] }
        } }"#);
        let err = resolve_order(&p, "a").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cycle"), "{msg}");
        assert!(msg.contains("a") && msg.contains("b"), "{msg}");
    }

    #[test]
    fn self_cycle_is_a_diagnostic() {
        let p = pkg(r#"{ scripts: { a: { run: "echo a", dependsOn: ["a"] } } }"#);
        assert!(matches!(resolve_order(&p, "a"), Err(RunError::Cycle(_))));
    }

    #[test]
    fn unknown_dep_names_the_task() {
        let p = pkg(r#"{ scripts: {
            a: { run: "echo a", dependsOn: ["ghost"] }
        } }"#);
        let err = resolve_order(&p, "a").unwrap_err();
        assert!(err.to_string().contains("`a`"), "{err}");
        assert!(err.to_string().contains("`ghost`"), "{err}");
    }

    #[test]
    fn unknown_script_lists_available() {
        let p = pkg(r#"{ scripts: { alpha: "echo a", beta: "echo b" } }"#);
        let err = resolve_order(&p, "gamma").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("gamma"), "{msg}");
        assert!(msg.contains("alpha") && msg.contains("beta"), "{msg}");
    }
}
