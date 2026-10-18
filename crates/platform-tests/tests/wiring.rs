//! Workspace wiring invariants — the dead-code / "idea ledger" gate.
//!
//! The kernel principle (ADR-0040) is that Rust holds the kernel plus generic
//! tooling, and everything else is Handl over a C ABI / SDK-gen contract. An
//! unwired Rust crate is therefore a *wrongly placed* idea, not just dead
//! weight — and the failure mode this guards against is an idea going dark
//! silently.
//!
//! `project/parked.yaml` is the durable index of retired ideas. These tests
//! keep the workspace and the ledger honest:
//!
//! 1. a crate listed as retired must no longer be a workspace member;
//! 2. every workspace member must be depended on by a sibling, or declared an
//!    entry point in the ledger (this is the anti-orphan regression gate);
//! 3. every ledger entry must point at something real (no stale entries).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Ledger {
    version: u32,
    entry_points: Vec<EntryPoint>,
    #[serde(default)]
    exempt: Vec<EntryPoint>,
    retired: Vec<Retired>,
}

#[derive(Debug, Deserialize)]
struct EntryPoint {
    #[serde(rename = "crate")]
    krate: String,
    why: String,
}

#[derive(Debug, Deserialize)]
struct Retired {
    was: String,
    idea: String,
    adr: String,
    status: String,
    #[serde(default)]
    superseded_by: Option<String>,
    #[serde(default)]
    unpark: Option<String>,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn ledger() -> Ledger {
    let path = repo_root().join("project/parked.yaml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_yaml::from_str(&text)
        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()))
}

/// The `name` field of a manifest's `[package]` section.
fn pkg_name(manifest: &Path) -> Option<String> {
    let text = std::fs::read_to_string(manifest).ok()?;
    let start = text.find("[package]")?;
    let section = &text[start + "[package]".len()..];
    let end = section.find("\n[").unwrap_or(section.len());
    for line in section[..end].lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("name") {
            let rest = rest.trim_start().strip_prefix('=')?.trim();
            let rest = rest.strip_prefix('"')?;
            return rest.split('"').next().map(str::to_string);
        }
    }
    None
}

/// Dependency keys declared by a manifest, across all dependency tables.
fn dep_keys(manifest: &Path, known: &BTreeSet<String>) -> BTreeSet<String> {
    let Ok(text) = std::fs::read_to_string(manifest) else {
        return BTreeSet::new();
    };
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim();
        // Table form: `[dependencies.foo]` / `[dev-dependencies.foo]`.
        if let Some(inner) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
                if let Some(name) = inner.strip_prefix(table).and_then(|r| r.strip_prefix('.')) {
                    if known.contains(name) {
                        out.insert(name.to_string());
                    }
                }
            }
            continue;
        }
        // Key form: `foo = { ... }` or `foo.workspace = true`.
        let Some((key, _)) = line.split_once('=') else {
            continue;
        };
        let mut key = key.trim();
        if key.is_empty() || key == "name" || key.contains(' ') {
            continue;
        }
        if let Some((head, _)) = key.split_once('.') {
            key = head;
        }
        if known.contains(key) {
            out.insert(key.to_string());
        }
    }
    out
}

/// Every workspace member: root package + the root manifest's `members` paths.
fn members(root: &Path) -> BTreeMap<String, PathBuf> {
    let text = std::fs::read_to_string(root.join("Cargo.toml")).expect("root Cargo.toml");
    let mstart = text.find("members").expect("members key");
    let open = mstart + text[mstart..].find('[').expect("members [");
    let close = open + text[open..].find(']').expect("members ]");
    let body = &text[open + 1..close];

    let mut dirs = vec![root.to_path_buf()];
    for part in body.split('"').skip(1).step_by(2) {
        dirs.push(root.join(part));
    }

    let mut out = BTreeMap::new();
    for dir in dirs {
        if let Some(name) = pkg_name(&dir.join("Cargo.toml")) {
            out.insert(name, dir);
        }
    }
    out
}

#[test]
fn retired_crates_are_no_longer_members() {
    let members = members(&repo_root());
    let mut offenders = vec![];
    for r in &ledger().retired {
        if members.contains_key(&r.was) {
            offenders.push(format!("{} (retired, {})", r.was, r.adr));
        }
    }
    assert!(
        offenders.is_empty(),
        "crates are marked retired in project/parked.yaml but are still workspace members:\n  {}\n\n\
         Delete the crate and remove it from the root Cargo.toml `members` list — keeping it \
         compiled keeps the wrong reification alive.",
        offenders.join("\n  ")
    );
}

#[test]
fn every_member_is_wired_or_declared() {
    let root = repo_root();
    let members = members(&root);
    let known: BTreeSet<String> = members.keys().cloned().collect();

    // Which members are depended on by some *other* member?
    let mut depended_on: BTreeSet<String> = BTreeSet::new();
    for (name, dir) in &members {
        for dep in dep_keys(&dir.join("Cargo.toml"), &known) {
            if &dep != name {
                depended_on.insert(dep);
            }
        }
    }

    let declared: BTreeSet<String> = ledger()
        .entry_points
        .iter()
        .chain(ledger().exempt.iter())
        .map(|e| e.krate.clone())
        .collect();

    let orphans: Vec<String> = members
        .keys()
        .filter(|n| !depended_on.contains(*n) && !declared.contains(*n))
        .cloned()
        .collect();

    assert!(
        orphans.is_empty(),
        "workspace members that nothing depends on and that are not declared entry points:\n  {}\n\n\
         Wire it up, retire it (add a `retired:` entry to project/parked.yaml and delete the crate), \
         or declare it under `entry_points:` with a reason.",
        orphans.join("\n  ")
    );
}

#[test]
fn ledger_is_not_stale() {
    let members = members(&repo_root());
    let l = ledger();

    assert_eq!(l.version, 1, "unexpected ledger version");

    let mut problems = vec![];
    let mut seen = BTreeSet::new();

    for e in l.entry_points.iter().chain(l.exempt.iter()) {
        if !members.contains_key(&e.krate) {
            problems.push(format!("entry point `{}` is not a workspace member", e.krate));
        }
        if e.why.trim().is_empty() {
            problems.push(format!("entry point `{}` has an empty reason", e.krate));
        }
        if !seen.insert(e.krate.clone()) {
            problems.push(format!("`{}` is listed twice in the ledger", e.krate));
        }
    }

    for r in &l.retired {
        if !seen.insert(r.was.clone()) {
            problems.push(format!("`{}` is listed twice in the ledger", r.was));
        }
        if r.idea.trim().is_empty() {
            problems.push(format!("retired `{}` has no `idea`", r.was));
        }
        if !r.adr.starts_with("ADR-") {
            problems.push(format!("retired `{}` has a malformed `adr`", r.was));
        }
        if r.unpark.as_deref().unwrap_or("").trim().is_empty() {
            problems.push(format!("retired `{}` has no `unpark` note", r.was));
        }
        if r.status != "proposed" && r.superseded_by.as_deref().unwrap_or("").trim().is_empty() {
            problems.push(format!(
                "retired `{}` has status `{}` but no `superseded_by`",
                r.was, r.status
            ));
        }
        if !adr_exists(&r.adr) {
            problems.push(format!(
                "retired `{}` cites `{}` but no such ADR file exists",
                r.was, r.adr
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "project/parked.yaml has stale or incomplete entries:\n  {}",
        problems.join("\n  ")
    );
}

/// Every `#[ignore]` must carry a reason, and a non-trivial one.
///
/// A bare `#[ignore]` is a silent skip: nothing records whether it is a
/// deliberate park or an oversight, so nobody can tell later. This repo carried
/// 172 ignored tests under one blanket `"n-dim"` label for an entire feature
/// transition — enough to look like blocked work when it was finished work.
/// Requiring a reason makes each skip a declaration.
#[test]
fn every_ignored_test_states_a_reason() {
    // Built at runtime so this test's own source does not match the needle.
    let needle = concat!("#[ignore", "]");
    let root = repo_root();
    let mut offenders = vec![];
    visit(&root.join("crates"), &mut |path| {
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            return;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        for (idx, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            if line.replace(' ', "").contains(needle) {
                let rel = path.strip_prefix(&root).unwrap_or(path);
                offenders.push(format!("{}:{}", rel.display(), idx + 1));
            }
        }
    });
    assert!(
        offenders.is_empty(),
        "ignored tests without a reason — a bare `{needle}` records nothing about whether the skip \
         is a deliberate park or an oversight:\n  {}\n\n\
         Write `#[ignore = \"why, and what would un-ignore it\"]`.",
        offenders.join("\n  ")
    );
}

/// Recursively visit every file under `dir`.
fn visit(dir: &Path, f: &mut impl FnMut(&Path)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            visit(&path, f);
        } else {
            f(&path);
        }
    }
}

/// A module behind an empty `cfg(any())` is **never compiled** — invisible to
/// the compiler *and* to coverage, so nothing can report it. One such block hid
/// 458 lines of pre-ADR-0119 tests in `hnl-engine/src/term_data.rs` until it
/// was found by hand.
///
/// Skipping a compilation is therefore a decision that must be declared, not
/// absorbed: delete the code, or park the idea in `project/parked.yaml`.
#[test]
fn no_silently_disabled_compilation() {
    // Built at runtime so this test's own source does not match the needle.
    let needle = concat!("cfg(", "any())");
    let root = repo_root();
    let mut offenders = vec![];
    visit(&root.join("crates"), &mut |path| {
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            return;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        for (idx, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue; // prose is allowed to discuss the pattern
            }
            if line.replace(' ', "").contains(needle) {
                let rel = path.strip_prefix(&root).unwrap_or(path);
                offenders.push(format!("{}:{}", rel.display(), idx + 1));
            }
        }
    });
    assert!(
        offenders.is_empty(),
        "compilation is silently disabled — an empty `{needle}` block is never compiled, so \
         neither the compiler nor coverage can see it:\n  {}\n\n\
         Delete the code, or park the idea in project/parked.yaml.",
        offenders.join("\n  ")
    );
}

/// ADRs are filed as `NNNN-slug.md`, so cite by number and check the prefix.
/// Archived ADRs count — they are still the record of an idea.
///
/// ADRs live on the `project` branch, mounted at `project/` as a worktree
/// (ADR-0129). In a plain clone or CI that directory is absent, so the check
/// degrades to "cannot verify here" instead of failing every ledger entry.
fn adr_exists(adr: &str) -> bool {
    let number = adr.trim_start_matches("ADR-");
    let base = repo_root().join("project/adrs");
    if !base.is_dir() {
        return true;
    }
    [base.clone(), base.join("archived")].iter().any(|dir| {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        entries.filter_map(Result::ok).any(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with(number) && name.ends_with(".md")
        })
    })
}
