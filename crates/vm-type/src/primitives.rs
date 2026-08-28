use crate::core::{Env, Kind, Literal, Name, TCtx, Value};

/// Returns an environment containing the base `type` collections.
///
/// The bindings are stored by De Bruijn index in the order:
/// `type.number`, `type.string`, `type.bool`, `type.null`, `type.array`,
/// `type.object`, `type.eq`.  Because `Env::extend` inserts at the front,
/// the values are inserted in reverse so that `type.number` is at index 0.
pub fn primitive_env() -> Env {
    Env::new()
        .extend(Value::Collection(Name::new("type.eq")))
        .extend(Value::Collection(Name::new("type.object")))
        .extend(Value::Collection(Name::new("type.array")))
        .extend(Value::Collection(Name::new("type.null")))
        .extend(Value::Collection(Name::new("type.bool")))
        .extend(Value::Collection(Name::new("type.string")))
        .extend(Value::Collection(Name::new("type.number")))
}

/// Alias for `primitive_env`.
pub fn prelude_env() -> Env {
    primitive_env()
}

/// Alias for `primitive_env`.
pub fn prelude() -> Env {
    primitive_env()
}

/// Returns a typing context with the base `type` collections bound as types
/// of kind `Type`.
pub fn prelude_tctx() -> TCtx {
    TCtx::new()
        .extend_with_kind(
            Name::new("type.eq"),
            Value::Collection(Name::new("type.eq")),
            Kind::Type,
        )
        .extend_with_kind(
            Name::new("type.object"),
            Value::Collection(Name::new("type.object")),
            Kind::Type,
        )
        .extend_with_kind(
            Name::new("type.array"),
            Value::Collection(Name::new("type.array")),
            Kind::Type,
        )
        .extend_with_kind(
            Name::new("type.null"),
            Value::Collection(Name::new("type.null")),
            Kind::Type,
        )
        .extend_with_kind(
            Name::new("type.bool"),
            Value::Collection(Name::new("type.bool")),
            Kind::Type,
        )
        .extend_with_kind(
            Name::new("type.string"),
            Value::Collection(Name::new("type.string")),
            Kind::Type,
        )
        .extend_with_kind(
            Name::new("type.number"),
            Value::Collection(Name::new("type.number")),
            Kind::Type,
        )
}

/// Returns `Some(true)` if `value` is a member of the named primitive
/// collection, `Some(false)` if it is definitely not, and `None` if the
/// collection is not a known primitive or membership cannot be decided.
pub fn primitive_member(value: &Value, collection: &Name) -> Option<bool> {
    match collection.0.as_str() {
        "type.number" => matches!(value, Value::Literal(Literal::Int(_))),
        "type.string" => matches!(value, Value::Literal(Literal::String(_))),
        "type.bool" => matches!(value, Value::Literal(Literal::Bool(_))),
        "type.null" => matches!(value, Value::Null),
        "type.array" => matches!(value, Value::Array(_)),
        "type.object" => matches!(value, Value::Object(_)),
        "type.eq" => matches!(value, Value::Refl),
        _ => return None,
    }
    .into()
}
