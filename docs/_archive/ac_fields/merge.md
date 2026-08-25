**handl_fields > merge**

# Module: merge

## Contents

**Functions**

- [`merge_fields`](#merge_fields) - Spreads `merge: true` field values into the parent scope, then removes

---

## handl_fields::merge::merge_fields

*Function*

Spreads `merge: true` field values into the parent scope, then removes
the field itself.

After computation, fields declared with `merge: true` have their values
(which must be objects) flattened into the parent data object. The field
itself is removed after the spread.

Conflict resolution: parent wins — keys already present in `data` are
not overwritten by merged values. Multiple `merge: true` fields merge in
declaration order; earlier merges fill keys that later merges cannot
overwrite.

Top-level spread only — no deep merge. If a merge field's value is not
an object, an error is returned for that field.

Returns a list of error messages for any merge fields that could not
be spread.

```rust
fn merge_fields(data: & mut serde_json::Value, merge: &[String]) -> Vec<String>
```



