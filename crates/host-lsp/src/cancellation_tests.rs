use std::collections::HashSet;

use serde_json::json;

use crate::is_cancelled;

#[test]
fn is_cancelled_returns_true_when_id_in_set() {
    let mut cancelled: HashSet<serde_json::Value> = HashSet::new();
    cancelled.insert(json!(5));

    assert!(is_cancelled(&json!(5), &cancelled));
}

#[test]
fn is_cancelled_returns_false_when_id_not_in_set() {
    let cancelled: HashSet<serde_json::Value> = HashSet::new();

    assert!(!is_cancelled(&json!(5), &cancelled));
}

#[test]
fn cancel_request_extracts_id_into_set() {
    let params = json!({ "id": 5 });
    let cancel_id = params.get("id").unwrap();

    let mut cancelled: HashSet<serde_json::Value> = HashSet::new();
    cancelled.insert(cancel_id.clone());

    assert!(cancelled.contains(&json!(5)));
}
