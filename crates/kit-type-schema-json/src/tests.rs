use crate::*;
use handl_kit_type::*;
use serde_json::json;

fn prim_ctx() -> TCtx {
    let mut tctx = TCtx::new();
    for name in ["String", "Number", "Int", "Bool", "Null", "Object", "Unit"] {
        tctx = tctx.extend_with_kind(Name::new(name), named_ty(name), Kind::Type);
    }
    tctx
}

#[test]
fn primitive_types() {
    let root = json!({});
    assert_eq!(
        interpret(&json!({"type": "string"}), &root).unwrap(),
        tvar("String")
    );
    assert_eq!(
        interpret(&json!({"type": "number"}), &root).unwrap(),
        tvar("Number")
    );
    assert_eq!(
        interpret(&json!({"type": "integer"}), &root).unwrap(),
        tvar("Int")
    );
    assert_eq!(
        interpret(&json!({"type": "boolean"}), &root).unwrap(),
        tvar("Bool")
    );
    assert_eq!(
        interpret(&json!({"type": "null"}), &root).unwrap(),
        tvar("Null")
    );
}

#[test]
fn array_type() {
    let root = json!({});
    let term = interpret(
        &json!({"type": "array", "items": {"type": "string"}}),
        &root,
    )
    .unwrap();
    assert_eq!(term, type_app(tvar("Array"), tvar("String")));
}

#[test]
fn object_record() {
    let root = json!({});
    let term = interpret(
        &json!({
            "type": "object",
            "properties": {
                "x": {"type": "integer"},
                "flag": {"type": "boolean"}
            }
        }),
        &root,
    )
    .unwrap();
    // BTreeMap order: "flag" before "x"; wrapped from inner to outer.
    let expected = sigma("flag", tvar("Bool"), sigma("x", tvar("Int"), u0()));
    assert_eq!(term, expected);
}

#[test]
fn function_with_type_params() {
    let root = json!({});
    let term = interpret(
        &json!({
            "type": "function",
            "typeParams": ["T"],
            "params": [{"name": "x", "schema": {"type": "var", "name": "T"}}],
            "return": {"type": "var", "name": "T"}
        }),
        &root,
    )
    .unwrap();
    let expected = type_lam("T", Kind::Type, pi("x", bvar(0), bvar(1)));
    assert_eq!(term, expected);
}

#[test]
fn generic_map_alias() {
    let root = json!({
        "$defs": {
            "Map": {
                "typeParams": ["T", "U"],
                "type": "object",
                "properties": {
                    "key": {"type": "var", "name": "T"},
                    "value": {"type": "var", "name": "U"}
                }
            }
        }
    });
    let schema = json!({"$ref": "#/$defs/Map", "args": [{"type": "string"}, {"type": "integer"}]});
    let term = interpret(&schema, &root).unwrap();

    let expected = sigma("key", tvar("String"), sigma("value", tvar("Int"), u0()));
    assert_eq!(normalize(&term), expected);
}

#[test]
fn ref_without_args() {
    let root = json!({
        "$defs": {
            "StringAlias": {"type": "string"}
        }
    });
    let term = interpret(&json!({"$ref": "#/$defs/StringAlias"}), &root).unwrap();
    assert_eq!(term, tvar("String"));
}

#[test]
fn var_in_items_and_properties() {
    let root = json!({
        "$defs": {
            "Container": {
                "typeParams": ["T"],
                "type": "object",
                "properties": {
                    "items": {"type": "array", "items": {"type": "var", "name": "T"}}
                }
            }
        }
    });
    let schema = json!({"$ref": "#/$defs/Container", "args": [{"type": "number"}]});
    let term = interpret(&schema, &root).unwrap();
    let expected = sigma("items", type_app(tvar("Array"), tvar("Number")), u0());
    assert_eq!(normalize(&term), expected);
}

#[test]
fn all_of_same() {
    let root = json!({});
    let term = interpret(
        &json!({"allOf": [{"type": "string"}, {"type": "string"}]}),
        &root,
    )
    .unwrap();
    assert_eq!(term, tvar("String"));
}

#[test]
fn any_of_same_and_mixed() {
    let root = json!({});
    let same = interpret(
        &json!({"anyOf": [{"type": "string"}, {"type": "string"}]}),
        &root,
    )
    .unwrap();
    assert_eq!(same, tvar("String"));

    let mixed = interpret(
        &json!({"anyOf": [{"type": "string"}, {"type": "number"}]}),
        &root,
    )
    .unwrap();
    assert!(matches!(mixed, Term::Meta(_)));
}

#[test]
fn one_of_same_and_mixed() {
    let root = json!({});
    let same = interpret(
        &json!({"oneOf": [{"type": "string"}, {"type": "string"}]}),
        &root,
    )
    .unwrap();
    assert_eq!(same, tvar("String"));

    let mixed = interpret(
        &json!({"oneOf": [{"type": "string"}, {"type": "number"}]}),
        &root,
    )
    .unwrap();
    assert!(matches!(mixed, Term::Meta(_)));
}

#[test]
fn not_is_unknown() {
    let root = json!({});
    let term = interpret(&json!({"not": {"type": "string"}}), &root).unwrap();
    assert!(matches!(term, Term::Meta(_)));
}

#[test]
fn if_then_else_branches() {
    let root = json!({});
    let same = interpret(
        &json!({
            "if": {"type": "string"},
            "then": {"type": "string"},
            "else": {"type": "string"}
        }),
        &root,
    )
    .unwrap();
    assert_eq!(same, tvar("String"));

    let diff = interpret(
        &json!({
            "if": {"type": "string"},
            "then": {"type": "string"},
            "else": {"type": "number"}
        }),
        &root,
    )
    .unwrap();
    assert!(matches!(diff, Term::Meta(_)));
}

#[test]
fn unknown_type_is_meta() {
    let root = json!({});
    let term = interpret(&json!({"type": "frob"}), &root).unwrap();
    assert!(matches!(term, Term::Meta(_)));
}

#[test]
fn round_trip_primitive() {
    let root = json!({});
    let t = interpret(&json!({"type": "string"}), &root).unwrap();
    let tctx = prim_ctx();
    let mut ctx = MetaContext::new();
    let ty = synth(&mut ctx, &Env::new(), &tctx, &t).unwrap();
    let v = eval(&t, &Env::new());
    let q = quote(&v, &[]);
    assert_eq!(q, t);
    assert_eq!(quote(&ty, &[]), universe(Level::zero()));
}

#[test]
fn round_trip_function_type() {
    let root = json!({});
    let t = interpret(
        &json!({
            "type": "function",
            "params": [{"name": "x", "schema": {"type": "string"}}],
            "return": {"type": "integer"}
        }),
        &root,
    )
    .unwrap();
    let tctx = prim_ctx();
    let mut ctx = MetaContext::new();
    let ty = synth(&mut ctx, &Env::new(), &tctx, &t).unwrap();
    let v = eval(&t, &Env::new());
    let q = quote(&v, &[]);
    assert_eq!(q, t);
    assert_eq!(quote(&ty, &[]), universe(Level::zero()));
}

#[test]
fn round_trip_generic_map() {
    let root = json!({
        "$defs": {
            "Map": {
                "typeParams": ["T", "U"],
                "type": "object",
                "properties": {
                    "key": {"type": "var", "name": "T"},
                    "value": {"type": "var", "name": "U"}
                }
            }
        }
    });
    let t = interpret(
        &json!({"$ref": "#/$defs/Map", "args": [{"type": "string"}, {"type": "integer"}]}),
        &root,
    )
    .unwrap();
    let tctx = prim_ctx();
    let mut ctx = MetaContext::new();
    let _ = synth_type(&mut ctx, &Env::new(), &tctx, &t).unwrap();
    let v = eval(&t, &Env::new());
    let q = quote(&v, &[]);
    assert_eq!(q, normalize(&t));
}

#[test]
fn repeated_ref_is_cached() {
    let root = json!({
        "$defs": {
            "X": {"not": {"type": "string"}}
        }
    });
    let schema = json!({"allOf": [{"$ref": "#/$defs/X"}, {"$ref": "#/$defs/X"}]});
    let term = interpret(&schema, &root).unwrap();
    assert_eq!(term, meta(0));
}

#[test]
fn arg_count_mismatch_is_error() {
    let root = json!({
        "$defs": {
            "Map": {
                "typeParams": ["T", "U"],
                "type": "object",
                "properties": {
                    "key": {"type": "var", "name": "T"},
                    "value": {"type": "var", "name": "U"}
                }
            }
        }
    });
    let err = interpret(
        &json!({"$ref": "#/$defs/Map", "args": [{"type": "string"}]}),
        &root,
    )
    .unwrap_err();
    assert!(matches!(err, SchemaError::ArgCountMismatch { .. }));
}

#[test]
fn invalid_ref_prefix_is_unknown() {
    let err = interpret(&json!({"$ref": "#/elsewhere"}), &json!({})).unwrap_err();
    assert!(matches!(err, SchemaError::UnknownRef(_)));
}

#[test]
fn invalid_function_param_is_error() {
    let err = interpret(
        &json!({
            "type": "function",
            "params": ["not an object"]
        }),
        &json!({}),
    )
    .unwrap_err();
    assert!(matches!(err, SchemaError::InvalidSchema(_)));
}

#[test]
fn invalid_var_is_error() {
    let err = interpret(&json!({"type": "var", "name": 42}), &json!({})).unwrap_err();
    assert!(matches!(err, SchemaError::InvalidSchema(_)));
}

#[test]
fn recursive_ref_returns_unknown() {
    let root = json!({
        "$defs": {
            "Loop": {"$ref": "#/$defs/Loop"}
        }
    });
    let term = interpret(&json!({"$ref": "#/$defs/Loop"}), &root).unwrap();
    assert!(matches!(term, Term::Meta(_)));
}

#[test]
fn all_of_empty_is_unknown() {
    let term = interpret(&json!({"allOf": []}), &json!({})).unwrap();
    assert!(matches!(term, Term::Meta(_)));
}

#[test]
fn additional_properties_record() {
    let term = interpret(
        &json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            },
            "additionalProperties": {"type": "integer"}
        }),
        &json!({}),
    )
    .unwrap();
    assert!(matches!(term, Term::Sigma { .. }));
}

#[test]
fn interpret_non_object_returns_unknown() {
    let term = interpret(&json!("string"), &json!({})).unwrap();
    assert!(matches!(term, Term::Meta(_)));
}

#[test]
fn schema_error_display_and_debug() {
    let e = SchemaError::InvalidSchema("bad".into());
    assert!(e.to_string().contains("invalid schema"));
    assert_eq!(format!("{:?}", e), "InvalidSchema(\"bad\")");
    let e = SchemaError::RecursionLimit("x".into());
    assert!(e.to_string().contains("recursion"));
}

#[test]
fn round_trip_record() {
    let root = json!({});
    let t = interpret(
        &json!({
            "type": "object",
            "properties": {
                "x": {"type": "integer"},
                "flag": {"type": "boolean"}
            }
        }),
        &root,
    )
    .unwrap();
    let tctx = prim_ctx();
    let mut ctx = MetaContext::new();
    let ty = synth(&mut ctx, &Env::new(), &tctx, &t).unwrap();
    let v = eval(&t, &Env::new());
    let q = quote(&v, &[]);
    assert_eq!(q, normalize(&t));
    assert_eq!(quote(&ty, &[]), universe(Level::zero().succ()));
}

#[test]
fn empty_object_is_object() {
    let term = interpret(&json!({"type": "object"}), &json!({})).unwrap();
    assert_eq!(term, tvar("Object"));
}

#[test]
fn function_param_defaults() {
    let term = interpret(
        &json!({
            "type": "function",
            "params": [{"schema": {"type": "string"}}],
            "return": {"type": "integer"}
        }),
        &json!({}),
    )
    .unwrap();
    assert!(matches!(term, Term::Pi { .. }));
}

#[test]
fn ref_must_be_string() {
    let err = interpret(&json!({"$ref": 1}), &json!({})).unwrap_err();
    assert!(matches!(err, SchemaError::InvalidSchema(_)));
}

#[test]
fn one_of_empty_is_unknown() {
    let term = interpret(&json!({"oneOf": []}), &json!({})).unwrap();
    assert!(matches!(term, Term::Meta(_)));
}
