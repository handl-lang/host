//! # handl-kit-eqsat
//!
//! The **eqsat law/cost corpus as kit data** — the KIT-layer home named
//! by ADR-0175's dissolution row (`eqsat → engine-eqsat-* + kit-eqsat`)
//! and fixed by ADR-0177 §1: laws, guards, and costs are kit *data* —
//! `LAW_FORMAT = 2` JSON records, never `.hnl` (one codec; a law is a
//! 2-cell, not a program). This repo is that kit repo; until it
//! existed, `engine-eqsat/laws/*.json` served as the reference corpus.
//!
//! ## What this crate is — and is not
//!
//! It is a **data kit**: [`LAW_PACKS`] embeds the corpus files under
//! `data/laws/` verbatim (`include_str!`) and hands consumers the JSON
//! text or parsed [`serde_json::Value`]s. It deliberately links **no**
//! handl crates — the interpret layer (`hnl-engine-eqsat`'s
//! `LawSet::from_json` / `CostSpec::from_data`) and the Python bridge
//! (`mlopt_orchestrator.eqsat.portable_laws`) each own their decoder;
//! this crate only owns the bytes. That keeps the dependency arrow
//! one-directional (interpret/Python → kit data) and makes the corpus
//! consumable from Rust, Python (via file/subprocess), and any future
//! `.hnl` kit manifest without a codec in between.
//!
//! ## The corpus
//!
//! `data/laws/*.json` holds the non-demo packs of the
//! `LAW_FORMAT = 2` record corpus — **524 records** across ten
//! packs:
//!
//! | pack | records | contents |
//! |---|---|---|
//! | `simplification` | 35 | the guard-free `SIMPLIFICATION_RULES` ports |
//! | `categorical` | 50 | the guard-free `CATEGORICAL_RULES` ports |
//! | `prior_art` | 171 | the portable `PRIOR_ART_RULES` slice (§1 TASO concat structure + matmul/linear/conv merges + index-view select/slice bridges, §1/§2 no-op elimination, reduction algebra, activation decompositions, const-algebra folds, §2 parity/scalar folds, spelled-unit folds, §3 zoo-corpus transforms) |
//! | `attention` | 44 | the opt-in `ATTENTION_RULES` + `SDPA_EXPAND_RULES` (rope/scale algebra + sdpa expand key-sets) |
//! | `pointwise_factor` | 2 | the opt-in unguarded factor laws |
//! | `batch_norm` | 3 | the opt-in eval-mode `BATCH_NORM_RULES` expand triple |
//! | `conv_folds` | 60 | the opt-in generated `CONV_FOLD_RULES` family |
//! | `scan` | 22 | the opt-in `SCAN_LAWS` + `SCAN_DIAG_LAWS` monoids |
//! | `layout` | 77 | the opt-in `LAYOUT_RULES` transpose/layout migration family |
//! | `prim_folds` | 60 | the minted `prim:synop*` fold records as data |
//!
//! `prim_folds` has no `builtin` constructor — it is minted-record
//! data whose synthetic `synop_*` operator denotations stay
//! domain-side; it lands here directly, never emitted by
//! `emit_laws`.
//!
//! `reshape_flatten_demo` is **excluded by design** — it is the UNSOUND
//! attrs-bind matcher demo, never a law pack. Procedural-hook laws
//! (`check=`/`derive=` Python callbacks) serialize with
//! `serializable: false` / non-empty `missing_hooks`; the honesty
//! fields travel in the records untouched (ADR-0177 §1).
//!
//! ## Costs
//!
//! `data/costs/` is the reserved home for [`CostSpec`](https://github.com/handl-lang/engine-eqsat)
//! records (`count`/`size`/`op_table` today; `flops` after shape
//! inference; roofline/fusion/executor as backend-profiled named
//! specs). No cost corpus has been minted yet — the directory lands
//! with the first consumer, not before.
//!
//! ## Minted records
//!
//! `minted.db` stays the ledger in `eqsat`; its rows are *projected*
//! into this record format at admission time (ADR-0177 §1). The
//! projection output lands here as additional `data/laws/` packs —
//! never as a second serialization of the store.
//!
//! ## Consumption story (who reads this today)
//!
//! * **`hnl-engine-eqsat`** — consumes the packs through this
//!   crate's `handl-kit-eqsat` dependency (`LAW_PACKS`/
//!   `law_pack(name).json` feed its contract tests);
//!   `engine-eqsat/laws/` is reduced to the demo pack
//!   (`reshape_flatten_demo`, engine-only by design). The corpus
//!   bytes canonical home is here; `engine-eqsat/examples/
//!   emit_laws.rs` remains the regeneration path from the Rust
//!   `builtin` constructors to `LAW_FORMAT = 2` records.
//! * **`mlopt_orchestrator.eqsat`** (Python bridge, ADR-0178 WP3) —
//!   reads `data/eqsat_laws.json`, now **generated from this repo**
//!   by eqsat's `scripts/emit_eqsat_laws.py`: it concatenates
//!   `data/laws/<pack>.json` in `LAW_PACKS` order and renders
//!   `json.dumps(records, indent=1, sort_keys=True)`, drift-guarded
//!   byte-exact by eqsat's `tests/test_eqsat_laws_corpus.py`. The
//!   contract across the boundary is *record-level* equality with
//!   [`corpus()`] — `corpus_json()`'s compact serde rendering is a
//!   convenience, not the on-disk spelling.
//! * **`hnl optimize --kit kit-eqsat`** (future) — the ADR-0175 run
//!   layer dispatches on `SearchEngine` with a `LawSet`/`CostSpec`
//!   built from this data; no `.hnl` surface spelling exists or is
//!   planned (ADR-0177 §1 rejects `.hnl` records — one codec).

#![warn(missing_docs)]

use serde_json::Value;

/// The kit name under which this kit registers itself — for kit
/// manifests and `extends` resolution once a `.hnl` manifest lands.
pub const KIT_NAME: &str = "kit-eqsat";

/// One embedded law pack: a pack name plus its `LAW_FORMAT = 2` JSON
/// text, verbatim from `data/laws/<name>.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LawPack {
    /// The pack name — the file stem under `data/laws/` and the
    /// `tags` family the records carry (e.g. `"simplification"`).
    pub name: &'static str,
    /// The pack's JSON text: a top-level array of law records.
    pub json: &'static str,
}

impl LawPack {
    /// Parse the pack's records.
    ///
    /// # Errors
    ///
    /// Returns [`serde_json::Error`] if the embedded text is not a
    /// JSON array — impossible for the committed corpus (the tests
    /// pin it), so this is a decode-failure surface, not a data
    /// condition.
    pub fn records(&self) -> Result<Vec<Value>, serde_json::Error> {
        serde_json::from_str(self.json)
    }
}

/// The shipped law packs, in corpus order. Excludes
/// `reshape_flatten_demo` by design (see the module docs).
pub const LAW_PACKS: &[LawPack] = &[
    LawPack {
        name: "simplification",
        json: include_str!("../data/laws/simplification.json"),
    },
    LawPack {
        name: "categorical",
        json: include_str!("../data/laws/categorical.json"),
    },
    LawPack {
        name: "prior_art",
        json: include_str!("../data/laws/prior_art.json"),
    },
    LawPack {
        name: "attention",
        json: include_str!("../data/laws/attention.json"),
    },
    LawPack {
        name: "pointwise_factor",
        json: include_str!("../data/laws/pointwise_factor.json"),
    },
    LawPack {
        name: "batch_norm",
        json: include_str!("../data/laws/batch_norm.json"),
    },
    LawPack {
        name: "conv_folds",
        json: include_str!("../data/laws/conv_folds.json"),
    },
    LawPack {
        name: "scan",
        json: include_str!("../data/laws/scan.json"),
    },
    LawPack {
        name: "layout",
        json: include_str!("../data/laws/layout.json"),
    },
    LawPack {
        name: "prim_folds",
        json: include_str!("../data/laws/prim_folds.json"),
    },
];

/// Look up a pack by name.
pub fn law_pack(name: &str) -> Option<&'static LawPack> {
    LAW_PACKS.iter().find(|p| p.name == name)
}

/// The names of all shipped packs, in [`LAW_PACKS`] order.
pub fn law_pack_names() -> impl Iterator<Item = &'static str> {
    LAW_PACKS.iter().map(|p| p.name)
}

/// The whole corpus as one flat record list — the record-level
/// content `mlopt_orchestrator.data/eqsat_laws.json` snapshots (pack
/// order, records in file order; 524 records). That file is generated
/// by eqsat's `scripts/emit_eqsat_laws.py` from the `data/laws/`
/// packs directly; equality with this list is the cross-repo
/// contract, not byte equality of any particular rendering.
///
/// # Errors
///
/// Propagates the first pack's [`serde_json::Error`].
pub fn corpus() -> Result<Vec<Value>, serde_json::Error> {
    let mut out = Vec::new();
    for pack in LAW_PACKS {
        out.extend(pack.records()?);
    }
    Ok(out)
}

/// The whole corpus serialized as a single compact JSON array
/// (`serde_json::to_string`) — record-equal to
/// `mlopt_orchestrator.data/eqsat_laws.json`, but **not** its
/// byte-exact spelling: eqsat's `scripts/emit_eqsat_laws.py` renders
/// the same records with `json.dumps(records, indent=1,
/// sort_keys=True)`, and byte-exactness of that artifact is owned by
/// the emitter plus eqsat's `tests/test_eqsat_laws_corpus.py` drift
/// guard, not by this function.
///
/// # Errors
///
/// Propagates the first pack's [`serde_json::Error`].
pub fn corpus_json() -> Result<String, serde_json::Error> {
    serde_json::to_string(&corpus()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_parse_and_total_524() {
        let total: usize = LAW_PACKS.iter().map(|p| p.records().unwrap().len()).sum();
        assert_eq!(total, 524, "corpus drifted from the 524-record snapshot");
    }

    #[test]
    fn record_names_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for pack in LAW_PACKS {
            for rec in pack.records().unwrap() {
                let name = rec["name"].as_str().expect("record name");
                assert!(seen.insert(name.to_string()), "duplicate law {name}");
                assert_eq!(rec["version"], 2);
                assert_eq!(rec["kind"], "law");
                assert!(rec["lhs"].is_object() && rec["rhs"].is_object());
            }
        }
    }

    #[test]
    fn corpus_is_flattened_packs() {
        let flat = corpus().unwrap();
        let per_pack: usize = LAW_PACKS.iter().map(|p| p.records().unwrap().len()).sum();
        assert_eq!(flat.len(), per_pack);
        // corpus_json round-trips to the same record count.
        let reparsed: Vec<Value> = serde_json::from_str(&corpus_json().unwrap()).unwrap();
        assert_eq!(reparsed.len(), per_pack);
    }

    #[test]
    fn lookup_by_name() {
        assert_eq!(law_pack("simplification").unwrap().name, "simplification");
        assert!(law_pack("reshape_flatten_demo").is_none());
        assert!(law_pack("nope").is_none());
    }
}
