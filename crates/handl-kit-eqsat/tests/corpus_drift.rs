//! Corpus guard: `kit-eqsat` is the canonical `LAW_FORMAT = 2` law
//! corpus (ADR-0177 §1), so this test pins its *internal*
//! consistency — the `LAW_PACKS` table, the `data/laws/*.json` files
//! on disk, and the record schema — rather than diffing against a
//! sibling engine checkout.
//!
//! What changed: `engine-eqsat` no longer vendors the packs (it
//! consumes them through the `handl-kit-eqsat` dep and keeps only
//! `reshape_flatten_demo.json`), so the old pack-by-pack record diff
//! compared 0/N packs and had no teeth. Cross-repo *semantic* drift
//! — the shipped `eqsat_laws.json` snapshot vs `corpus()` — is owned
//! by eqsat's `tests/test_eqsat_laws_corpus.py` against
//! `scripts/emit_eqsat_laws.py`; do not re-grow a byte-diff here.
//!
//! What this file still asserts:
//!
//! * every `data/laws/<name>.json` file is in `LAW_PACKS` and vice
//!   versa (no orphan files, no table entries without bytes), and each
//!   embedded `LawPack.json` is byte-identical to its file;
//! * every record carries the `LAW_FORMAT = 2` keyset with sane field
//!   types (`version`/`kind`/`name`/`law`/`lhs`/`rhs`/`tags`/
//!   `serializable`/`missing_hooks`);
//! * per-pack record counts match the documented corpus table;
//! * `reshape_flatten_demo` stays engine-eqsat-only: absent from
//!   `LAW_PACKS`, absent from `data/laws/`, and — when the
//!   `engine-eqsat` sibling checkout exists — the *only* `.json`
//!   under its `laws/` dir. The sibling is resolved relative to this
//!   crate (`kit-eqsat/crates/handl-kit-eqsat` →
//!   `../../../engine-eqsat`), which works in the main checkout and
//!   in a detached-worktree CI layout; when absent (standalone
//!   clone, published tarball) that one check skips with a note.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use handl_kit_eqsat::{LAW_PACKS, corpus, law_pack};

/// `data/laws/` relative to this crate's manifest dir.
fn data_laws_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data/laws")
}

/// `engine-eqsat/laws/` sibling, when the checkout exists.
fn sibling_laws_dir() -> Option<PathBuf> {
    // crates/handl-kit-eqsat → crates → kit-eqsat → <workspace parent>
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../engine-eqsat/laws");
    dir.is_dir().then_some(dir)
}

/// The documented per-pack record counts (AGENTS.md / README §"The
/// corpus"). A record moving between packs — or a silent add/remove —
/// trips this even when the 524 total in `lib.rs`'s unit tests holds.
const EXPECTED_COUNTS: &[(&str, usize)] = &[
    ("simplification", 35),
    ("categorical", 50),
    ("prior_art", 171),
    ("attention", 44),
    ("pointwise_factor", 2),
    ("batch_norm", 3),
    ("conv_folds", 60),
    ("scan", 22),
    ("layout", 77),
    ("prim_folds", 60),
];

#[test]
fn law_packs_table_matches_files_on_disk() {
    let dir = data_laws_dir();
    let on_disk: BTreeSet<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            assert!(
                name.ends_with(".json"),
                "non-JSON file in data/laws: {name}"
            );
            name.trim_end_matches(".json").to_string()
        })
        .collect();

    let in_table: BTreeSet<&str> = LAW_PACKS.iter().map(|p| p.name).collect();
    assert_eq!(
        in_table.len(),
        LAW_PACKS.len(),
        "duplicate pack name in LAW_PACKS"
    );
    let disk_set: BTreeSet<&str> = on_disk.iter().map(String::as_str).collect();
    assert_eq!(
        disk_set, in_table,
        "data/laws/ files and LAW_PACKS disagree — add the pack to both \
         or remove the orphan (see README §\"Adding a pack\")"
    );

    // Each embedded pack is byte-identical to its file — pins that the
    // include_str! path and the table name never diverge.
    for pack in LAW_PACKS {
        let text = std::fs::read_to_string(dir.join(format!("{}.json", pack.name))).unwrap();
        assert_eq!(
            pack.json, text,
            "LAW_PACKS[{}].json != data/laws/{}.json",
            pack.name, pack.name
        );
    }
}

#[test]
fn records_are_law_format_2() {
    for pack in LAW_PACKS {
        for rec in pack.records().unwrap() {
            let ctx = || format!("{} record {:?}", pack.name, rec["name"]);
            assert_eq!(rec["version"], 2, "{}: version", ctx());
            assert_eq!(rec["kind"], "law", "{}: kind", ctx());
            assert!(
                rec["name"].as_str().is_some_and(|s| !s.is_empty()),
                "{}: name",
                ctx()
            );
            // `law` is the human description — minted `prim_folds`
            // records legitimately carry "" (no prose was minted).
            assert!(rec["law"].is_string(), "{}: law", ctx());
            assert!(
                rec["lhs"].is_object() && rec["rhs"].is_object(),
                "{}: lhs/rhs",
                ctx()
            );
            assert!(
                rec["tags"]
                    .as_array()
                    .is_some_and(|t| { t.iter().all(|v| v.is_string()) }),
                "{}: tags",
                ctx()
            );
            // Honesty fields (ADR-0177 §1): `serializable` gates firing,
            // `missing_hooks` names the unported hooks. Both must be
            // present and well-typed even though today's corpus has
            // none flagged.
            assert!(rec["serializable"].is_boolean(), "{}: serializable", ctx());
            assert!(
                rec["missing_hooks"].as_array().is_some(),
                "{}: missing_hooks",
                ctx()
            );
        }
    }
}

#[test]
fn pack_record_counts_match_documented_table() {
    assert_eq!(LAW_PACKS.len(), EXPECTED_COUNTS.len());
    for (name, expected) in EXPECTED_COUNTS {
        let pack = law_pack(name).unwrap_or_else(|| panic!("LAW_PACKS missing {name}"));
        assert_eq!(
            pack.records().unwrap().len(),
            *expected,
            "pack {name} record count drifted"
        );
    }
    // And the flattened corpus agrees (524 total).
    assert_eq!(corpus().unwrap().len(), 524);
}

#[test]
fn reshape_flatten_demo_stays_engine_only() {
    assert!(law_pack("reshape_flatten_demo").is_none());
    // No demo file may leak into the corpus directory either — the
    // set-equality guard above would catch it, but name it here so a
    // failure reads as the design rule it is.
    assert!(
        !data_laws_dir().join("reshape_flatten_demo.json").exists(),
        "reshape_flatten_demo.json must not live in kit-eqsat data/laws"
    );

    let Some(dir) = sibling_laws_dir() else {
        eprintln!("engine-eqsat sibling not found — demo-location check skipped");
        return;
    };
    let files: BTreeSet<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".json"))
        .collect();
    assert_eq!(
        files,
        BTreeSet::from(["reshape_flatten_demo.json".to_string()]),
        "engine-eqsat/laws must contain only the demo pack — the corpus \
         packs live here now; re-vendored copies are drift"
    );
}
