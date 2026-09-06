//! Tests for `crate::implementation::implementation`.
//!
//! These build temporary `contracts/` + `arrows/` directory trees on disk so
//! that the refinement engine can load sibling arrow files and infer real
//! guarantees. The `insert` arrow graph (copied from the stdlib) is known to
//! infer a `Sorted(output)` guarantee, so it serves as a reliable
//! implementation of the `Sorted` axiom.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::implementation::implementation;

/// The `insert` arrow graph from the handl stdlib. The refinement engine
/// infers `Sorted(output)` for it (derived from the `cons` contract), making
/// it an implementation of the `Sorted` axiom.
const INSERT_HNL: &str = r#"-- insert: insert an element into a sorted list, maintaining sorted order.
rec {
  elem: input.0,
  acc:  input.1
} (
  state ~> match state.acc {
    []: return [state.elem],

    [h, ...t]: {
      cmp = prim.compare(state.elem, h)
      if cmp <= 0
        { out: [state.elem, ...state.acc] }
      else
        {
          rest_inserted = insert({ elem: state.elem, acc: t })
          {
            state: {
              elem: state.elem,
              acc:  [h, ...rest_inserted]
            }
          }
        }
    }
  }
)
"#;

/// A contract file declaring the `Sorted` axiom.
const SORTED_CONTRACT: &str = "axiom Sorted : List a -> List a\n  guarantee: Sorted(output)\n";

/// Monotonic counter so each test gets a unique temp directory, even when
/// tests run in parallel (where `SystemTime::now()` can return the same
/// nanosecond on separate threads).
static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Create a unique temporary directory for a test, returning its path.
fn make_temp_dir(prefix: &str) -> PathBuf {
    let n = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut base = std::env::temp_dir();
    base.push(format!(
        "handl-lsp-implementation-{}-{}-{}",
        prefix,
        std::process::id(),
        n,
    ));
    fs::create_dir_all(&base).expect("create temp dir");
    base
}

/// Build a temp tree with `contracts/sorted.hnl` (declaring `axiom Sorted`)
/// and `arrows/collection/insert.hnl` (the insert arrow that infers Sorted).
/// Returns the contract file path.
fn make_sorted_tree() -> PathBuf {
    let dir = make_temp_dir("sorted");
    let contracts_dir = dir.join("handl-packages").join("contracts");
    let arrows_dir = dir.join("handl-packages").join("collection");
    fs::create_dir_all(&contracts_dir).expect("create contracts dir");
    fs::create_dir_all(&arrows_dir).expect("create arrows/collection dir");

    let contract_path = contracts_dir.join("sorted.hnl");
    fs::write(&contract_path, SORTED_CONTRACT).expect("write sorted.hnl contract");

    let insert_path = arrows_dir.join("insert.hnl");
    fs::write(&insert_path, INSERT_HNL).expect("write insert.hnl arrow");

    contract_path
}

#[test]
fn implementation_for_axiom_name_finds_arrows() {
    let contract_path = make_sorted_tree();
    let contract_str = contract_path.to_string_lossy().to_string();

    // Offset pointing at the `Sorted` identifier in `axiom Sorted : …`.
    // "axiom Sorted" — `Sorted` starts at byte offset 6.
    let offset = SORTED_CONTRACT.find("Sorted").expect("find Sorted");

    let locations = implementation(SORTED_CONTRACT, offset, Some(&contract_str));

    assert!(
        !locations.is_empty(),
        "expected at least one implementation for the Sorted axiom"
    );

    // At least one location should point at insert.hnl, whose inferred
    // guarantee is Sorted(output).
    let any_insert = locations.iter().any(|loc| {
        loc.get("uri")
            .and_then(|u| u.as_str())
            .map(|u| u.ends_with("insert.hnl"))
            .unwrap_or(false)
    });
    assert!(
        any_insert,
        "expected insert.hnl among the implementations: {:?}",
        locations
    );

    // Every location should be a well-formed { uri, range } object.
    for loc in &locations {
        assert!(loc.get("uri").and_then(|u| u.as_str()).is_some());
        let range = loc.get("range").expect("range present");
        assert_eq!(range["start"]["line"].as_u64().unwrap(), 0);
        assert_eq!(range["start"]["character"].as_u64().unwrap(), 0);
        assert_eq!(range["end"]["line"].as_u64().unwrap(), 0);
        assert_eq!(range["end"]["character"].as_u64().unwrap(), 0);
    }
}

#[test]
fn implementation_for_non_identifier_returns_empty() {
    let contract_path = make_sorted_tree();
    let contract_str = contract_path.to_string_lossy().to_string();

    // Offset on the `:` after `axiom Sorted` — not an identifier.
    let colon_offset = SORTED_CONTRACT.find(':').expect("find colon");
    let locations = implementation(SORTED_CONTRACT, colon_offset, Some(&contract_str));
    assert!(
        locations.is_empty(),
        "non-identifier cursor should yield no implementations"
    );
}

#[test]
fn implementation_empty_source_returns_empty() {
    let locations = implementation("", 0, Some("/tmp/contracts/empty.hnl"));
    assert!(
        locations.is_empty(),
        "empty source should yield no implementations"
    );
}

#[test]
fn implementation_no_match_returns_empty() {
    let dir = make_temp_dir("nomatch");
    let contracts_dir = dir.join("handl-packages").join("contracts");
    let arrows_dir = dir.join("handl-packages").join("collection");
    fs::create_dir_all(&contracts_dir).expect("create contracts dir");
    fs::create_dir_all(&arrows_dir).expect("create arrows dir");

    // A contract for an axiom that no arrow implements.
    let contract = "axiom Mystery : a -> a\n  guarantee: True\n";
    let contract_path = contracts_dir.join("mystery.hnl");
    fs::write(&contract_path, contract).expect("write mystery.hnl");

    // An arrow whose inferred guarantee is True (id >>> id), not Mystery(...).
    let arrow_path = arrows_dir.join("identity.hnl");
    fs::write(&arrow_path, "id >>> id").expect("write identity.hnl");

    let contract_str = contract_path.to_string_lossy().to_string();
    let offset = contract.find("Mystery").expect("find Mystery");

    let locations = implementation(contract, offset, Some(&contract_str));
    assert!(
        locations.is_empty(),
        "an axiom no arrow satisfies should yield no implementations: {:?}",
        locations
    );
}
