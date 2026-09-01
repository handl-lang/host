//! # Host type registry (ADR-0054)
//!
//! ADR-0054 (Generic Typecheck Engine) treats type values as **tagged values
//! minted by a host type registry** — not bare `tag` values. A type value
//! carries a `__registry__` field
//! recording which registry minted it, so equality and matching can be
//! scoped to a single registry:
//!
//! ```json
//! {"__tag__":"type.collection","__registry__":"handl.canonical","name":"number"}
//! {"__tag__":"type.pi","__registry__":"handl.canonical","param":...,"body":...}
//! ```
//!
//! The `construct` judgment op is the `tag` op (ADR-0055) called against the
//! host type registry; `match_type` is `match_tag` on type tags. Equality is
//! structural equality **within the registry**.
//!
//! This module provides [`TypeRegistry`] — the host primitive that mints,
//! recognizes, and dispatches on registry-scoped type values.

use handl_language::{is_tag_value, REGISTRY_VALUE_KEY, TAG_VALUE_KEY, Value};

/// The `type.` prefix carried by every type-value tag name.
const TYPE_TAG_PREFIX: &str = "type.";

/// A host type registry (ADR-0054 — Generic Typecheck Engine).
///
/// A `TypeRegistry` mints **type values**: tagged values whose `__tag__`
/// starts with `"type."` and that carry a [`REGISTRY_VALUE_KEY`] field
/// identifying this registry. Type values are the currency of the
/// typechecker — `construct` mints them, `match_type` dispatches on them,
/// and equality is structural within a single registry.
///
/// The registry identity makes type values unforgeable across registries: a
/// type value minted by registry `"handl.canonical"` is not recognized by a
/// registry with a different id, so two independent typecheck hosts cannot
/// be confused for one another.
///
/// `TypeRegistry` is pure data — it holds only a `registry_id` string and
/// performs no global registration. Construct one per host typecheck
/// surface (e.g. the canonical handl typechecker uses `"handl.canonical"`).
///
/// [`REGISTRY_VALUE_KEY`]: handl_language::REGISTRY_VALUE_KEY
pub struct TypeRegistry {
    /// The registry identity stamped into every minted type value's
    /// `__registry__` field.
    registry_id: String,
}

impl TypeRegistry {
    /// Create a new registry with the given identity.
    ///
    /// `registry_id` is stamped into the `__registry__` field of every type
    /// value this registry mints (e.g. `"handl.canonical"`).
    pub fn new(registry_id: impl Into<String>) -> Self {
        TypeRegistry {
            registry_id: registry_id.into(),
        }
    }

    /// The registry identity (the value stamped into `__registry__`).
    ///
    /// This is the string a type value's `__registry__` field must match for
    /// the value to be recognized as belonging to this registry.
    pub fn registry_id(&self) -> &str {
        &self.registry_id
    }

    /// Mint a type value (ADR-0054 §"construct").
    ///
    /// Returns `{"__tag__": tag_name, "__registry__": <registry_id>,
    /// ...payload}` — a tagged value carrying this registry's identity. The
    /// `payload` object's fields become the type value's payload; a
    /// non-object payload (or `null`) is treated as empty.
    ///
    /// Returns `None` (forgery prevention, same discipline as `tag_value`)
    /// if:
    /// - `payload` is not an object or `null`, or
    /// - `payload` already contains a `__tag__` or `__registry__` key (a
    ///   minted type value cannot be re-tagged or re-registered through its
    ///   payload).
    pub fn construct(&self, tag_name: &str, payload: &Value) -> Option<Value> {
        let mut obj = match payload {
            Value::Object(map) => map.clone(),
            Value::Null => serde_json::Map::new(),
            _ => return None,
        };
        if obj.contains_key(TAG_VALUE_KEY) || obj.contains_key(REGISTRY_VALUE_KEY) {
            return None;
        }
        obj.insert(TAG_VALUE_KEY.to_string(), Value::from(tag_name));
        obj.insert(
            REGISTRY_VALUE_KEY.to_string(),
            Value::from(self.registry_id.clone()),
        );
        Some(Value::Object(obj))
    }

    /// Whether `v` is a type value minted by this registry.
    ///
    /// True iff `v` is a tagged value whose `__registry__` matches this
    /// registry's id and whose `__tag__` starts with `"type."`. A type value
    /// from a different registry, a non-`type.` tag, or a non-tagged value is
    /// rejected.
    pub fn is_type_value(&self, v: &Value) -> bool {
        if !is_tag_value(v) {
            return false;
        }
        let tag = v.get(TAG_VALUE_KEY).and_then(|t| t.as_str());
        let matches_prefix = tag
            .map(|t| t.starts_with(TYPE_TAG_PREFIX))
            .unwrap_or(false);
        if !matches_prefix {
            return false;
        }
        v.get(REGISTRY_VALUE_KEY)
            .and_then(|r| r.as_str())
            .map(|r| r == self.registry_id)
            .unwrap_or(false)
    }

    /// Read the type tag of a type value minted by this registry.
    ///
    /// Returns `Some(tag_name)` if [`is_type_value`](Self::is_type_value)`(v)`
    /// is true, else `None`.
    pub fn as_type_tag<'a>(&self, v: &'a Value) -> Option<&'a str> {
        if self.is_type_value(v) {
            v.get(TAG_VALUE_KEY).and_then(|t| t.as_str())
        } else {
            None
        }
    }

    /// Dispatch on the type tag of `v` (ADR-0054 §"match_type").
    ///
    /// Like `match_tag` but only dispatches if `v` is a type value from this
    /// registry. `cases` is an object mapping type tag names to case values.
    /// If `v` is a type value whose tag has an explicit case, that case is
    /// returned; otherwise `None` is returned — there is **no default-case
    /// fallback** (the caller handles non-matches, per ADR-0054).
    ///
    /// Returns `None` if `v` is not a type value from this registry, or if
    /// `v`'s type tag has no case in `cases`.
    pub fn match_type(&self, v: &Value, cases: &Value) -> Option<Value> {
        if !self.is_type_value(v) {
            return None;
        }
        let tag = self.as_type_tag(v)?;
        let cases = cases.as_object()?;
        cases.get(tag).cloned()
    }
}
