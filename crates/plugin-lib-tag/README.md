# handl-plugin-lib-tag

Tagged values — the inspection and dispatch surface of handl's
**nominal identity** primitive (ADR-0055, ADR-0054).

A tagged value is a JSON object carrying a reserved `__tag__` key:

```json
{ "__tag__": "ok", "value": 42 }
{ "__tag__": "type.collection", "__registry__": "handl.canonical", "name": "number" }
```

The `__tag__` **mint producers were deleted** with the `__tag__`/`{tag}`
bridge (ADR-0152 §Migration): user code can no longer mint tagged values —
`tag.tag` and `tag.construct` no longer exist. What remains is the
*inspection* surface: host-internal `__tag__` records (law/rewrite/cell
records, sort and type declarations minted by eval's native ops) stay
inspectable, and `tag.match_type` dispatches on the type tags of
registry-scoped type values. This crate is that surface.

## API

The `tag` interface is declared in `lib-tag`'s `src/index.hnl` (the
`handl_lib_tag::TAG_HNL` source, `op` declarations) and backed by Rust
handlers registered in an [`OpRegistry`]:

| Callable            | Role                                                                    |
| ------------------- | ----------------------------------------------------------------------- |
| `tag.untag(v)`      | payload of `v` (`__tag__` removed); `null` for a non-tagged value       |
| `tag.tag_of(v)`     | tag name string; `null` for a non-tagged value                         |
| `tag.is_tag(v)`     | `true` iff `v` carries a string `__tag__`                               |
| `tag.match_type(v, cases)` | **dispatch** on a type value's tag — `cases` maps tag names to values; `null` on miss, no default fallback (ADR-0054) |

`tag.match_type` is backed by the [`TypeRegistry`] in `src/registry.rs`
(pure data, registry id `"handl.canonical"`) — the type-system machinery,
not a callable.

## Dispatch on tags

Tag dispatch on `__tag__` identity is handled by the native `match`
construct via tag patterns (`ok(v) => ...`), not a dedicated op
(ADR-0077). Combined with `error.try` this gives railway-style handling:

```handl
match error.try(_ ~> might_fail()) {
  ok(v): v,
  err(msg): "failed: " + msg,
}
```

## Rust surface

- `Untag`, `TagOf`, `IsTag`, `MatchType` — `Function` impls registered in
  `tag_op_registry()` under the `"tag.*"` op names.
- `TagLibraryPlugin` / `tag_handl_plugin()` — a `HandlLibraryPlugin`
  wrapper loading `tag.hnl` with the op registry. Registering it is the
  capability boundary: a host that does not register it does not offer
  the `tag` module at all (ADR-0055 §1).
- `TagModule` — legacy `Module` (`untag`, `tag_of`, `is_tag`,
  `match_type`) kept for downstream crates that register it directly.
- `TypeRegistry::new(id)` — `construct` / `is_type_value` / `as_type_tag` /
  `match_type` (pure data, no callable).
- `handl_language::{tag_value, as_tag, is_tag_value, is_reserved_value_key}`
  — mint/read/test primitives and the reserved-key guard (`__tag__`,
  `__registry__`, `__language__`, `__ast__`, `__arrow__`, `__home__`).
  `is_reserved_value_key` is re-exported from this crate.
