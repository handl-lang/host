use super::*;

#[test]
fn reexports_vm_type_items() {
    let _ = TCtx::new();
    let _ = MetaContext::new();
    let _ = Env::new();
    let _ = u0();
    let _ = tvar("String");
    let _ = Value::Universe(Level::zero());
}

#[test]
fn reexports_schema_bridge_items() {
    let _ = AliasTable::from_root(&serde_json::json!({}));
    let _ = SchemaError::UnknownRef("".into());
    let _ = default_tctx();
}

#[test]
fn typecheck_schema_primitive() {
    let schema = serde_json::json!({"type": "string"});
    let value = typecheck_schema(&schema).unwrap();
    assert_eq!(value, named_ty("String"));
}

#[test]
fn typecheck_schema_array_of_primitive() {
    let schema = serde_json::json!({
        "type": "array",
        "items": {"type": "string"},
    });
    let value = typecheck_schema(&schema).unwrap();
    assert_eq!(value, apply_type(named_ty("Array"), named_ty("String")));
}

#[test]
fn typecheck_schema_rejects_unbound_var() {
    let schema = serde_json::json!({"type": "var", "name": "Unknown"});
    assert!(typecheck_schema(&schema).is_err());
}

#[test]
fn default_tctx_has_primitives() {
    let tctx = default_tctx();
    for name in ["String", "Number", "Int", "Bool", "Null", "Object", "Array"] {
        assert!(
            tctx.lookup_by_name(&Name::new(name)).is_some(),
            "missing primitive {name}"
        );
    }
}

#[test]
fn typecheck_schema_rejects_malformed() {
    assert!(typecheck_schema(&serde_json::json!({"type": "frob"})).is_err());
    assert!(typecheck_schema(&serde_json::json!({"type": "var"})).is_err());
    assert!(typecheck_schema(&serde_json::json!({"$ref": "#/$defs/Missing"})).is_err());
}

#[test]
fn typecheck_schema_round_trip_function() {
    let schema = serde_json::json!({
        "type": "function",
        "params": [{"name": "x", "schema": {"type": "string"}}],
        "return": {"type": "integer"}
    });
    let value = typecheck_schema(&schema).unwrap();
    assert_eq!(quote(&value, &[]), pi("x", var("String"), var("Int")));
}

#[test]
fn typecheck_schema_with_local_defs() {
    let schema = serde_json::json!({
        "$defs": {
            "Box": {
                "typeParams": ["T"],
                "type": "object",
                "properties": {
                    "value": {"type": "var", "name": "T"}
                }
            }
        },
        "$ref": "#/$defs/Box",
        "args": [{"type": "string"}]
    });
    let value = typecheck_schema(&schema).unwrap();
    assert!(matches!(value, Value::Sigma { .. }));
}

#[test]
fn error_and_schema_debug_display() {
    let terr = TypeError::CannotSynthesize("test".into());
    assert_eq!(terr.to_string(), "cannot synthesize type for test");
    assert_eq!(format!("{:?}", terr), "CannotSynthesize(\"test\")");
    let serr = SchemaError::UnknownRef("x".into());
    assert_eq!(serr.to_string(), "unknown $ref: x");
}
