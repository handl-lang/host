//! `collections` ops — native atoms for the collections package.
//!
//! Ported verbatim from the deleted `plugin-lib-collections` crate:
//! - [`seq`] — the shared sequential-container ops (`new`/`from`/
//!   `to_array`/`len`/`is_empty` plus the end-parameterized
//!   `push`/`pop`/`peek` used by `vec`, `list`, `queue`, `stack`, and
//!   `deque`). All members except `new`/`from`/`to_array` are now pure
//!   handl (see `handl-packages/collections/{vec,list,queue,stack,deque}.hnl`);
//!   the `pop`/`peek`/`dequeue`/`pop_*`/`peek_*` contract requires raising
//!   `collection is empty` on an empty structure, which is now expressible
//!   via the `error.raise` primitive (`handl-packages/error/error.hnl`).
//!   `new` stays native because it is nullary and `from` is a reserved
//!   word (it cannot be a record field); `to_array` stays grouped with them.
//! - [`array`] — the `array.*` functional ops that take handl **arrows**
//!   (`take_while`/`drop_while`/`scan`/`foldr`/`find_*`): arrow arguments
//!   cannot cross the `HandlFunction` boundary, so they evaluate the
//!   serialized [`crate::function_value::FunctionValue`] body through
//!   `EvalContext::eval_expr` natively. The arrow-free `array.*` surface
//!   lives in `handl-packages/collections/array.hnl` as pure handl.
//! - [`hash_map`] — the `hash_map.*` object ops. Native because handl has
//!   no object-key-iteration primitive (`keys`/`values`/`entries`/
//!   `merge`/`pick`/`omit`/`invert` need to enumerate a record's fields)
//!   and several members take arrow arguments.
//! - [`hash_set`] — the `hash_set.*` set ops. Native because membership is
//!   keyed on a *serialized* value (`value_key`), which distinguishes
//!   `1` from `1.0` and differs from handl `==` (cross-numeric equality);
//!   a pure-handl port would change dedup semantics for mixed
//!   int/float inputs. `unique_by` additionally takes an arrow.
//!
//! [`collections_op_registry`] registers every op name under its full
//! `module.fn` name (matching the former `build_registry`); the
//! [`CollectionsModule`] aggregator preserves the former
//! `collections`-module lookup shape (`vec.new`, `collections.vec.new`).

use std::sync::Arc;

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::{HandlModule, ModuleResolver, OpRegistry};

// `#[allow(dead_code)]`: the `array`/`seq` impls whose surface moved to
// pure handl are kept compiled as the reference implementation; only the
// names registered in [`collections_op_registry`] are reachable.
#[allow(dead_code)]
mod array;
#[allow(dead_code)]
mod common;
mod error;
#[allow(dead_code)]
mod hash_map;
mod hash_set;
mod seq;

pub use error::CollectionsError;

// ---------------------------------------------------------------------------
// Op names per sub-module (from the former module.rs)
// ---------------------------------------------------------------------------

/// `vec.*` native op names — `new` is nullary and `from` is a reserved
/// word so it cannot be a handl record field (`to_array` stays grouped
/// with them); `len`/`is_empty`/`push`/`pop`/`peek` are pure handl in
/// `handl-packages/collections/vec.hnl`, with `pop`/`peek` raising
/// `collection is empty` via `error.raise`.
pub const VEC_OPS: &[&str] = &["vec.new", "vec.from", "vec.to_array"];

/// `list.*` native op names — same reasoning as [`VEC_OPS`]; `push`
/// (prepend), `pop`, and `peek` are pure handl.
pub const LIST_OPS: &[&str] = &["list.new", "list.from", "list.to_array"];

/// `queue.*` native op names — same reasoning as [`VEC_OPS`];
/// `dequeue`/`peek` raise on empty via `error.raise` in pure handl.
pub const QUEUE_OPS: &[&str] = &["queue.new", "queue.from", "queue.to_array"];

/// `stack.*` native op names — same reasoning as [`VEC_OPS`].
pub const STACK_OPS: &[&str] = &["stack.new", "stack.from", "stack.to_array"];

/// `deque.*` native op names — same reasoning as [`VEC_OPS`]; the
/// `pop_*`/`peek_*` members are pure handl over `error.raise`.
pub const DEQUE_OPS: &[&str] = &["deque.new", "deque.from", "deque.to_array"];

/// `hash_map.*` op names (all native).
pub const HASH_MAP_OPS: &[&str] = &[
    "hash_map.new",
    "hash_map.from",
    "hash_map.to_object",
    "hash_map.len",
    "hash_map.is_empty",
    "hash_map.get",
    "hash_map.set",
    "hash_map.remove",
    "hash_map.has",
    "hash_map.keys",
    "hash_map.values",
    "hash_map.entries",
    "hash_map.size",
    "hash_map.get_or",
    "hash_map.deep_get",
    "hash_map.path_or",
    "hash_map.update",
    "hash_map.set_path",
    "hash_map.update_path",
    "hash_map.map_values",
    "hash_map.map_keys",
    "hash_map.filter_keys",
    "hash_map.filter_values",
    "hash_map.invert",
    "hash_map.clone_deep",
    "hash_map.pick",
    "hash_map.omit",
    "hash_map.merge",
    "hash_map.deep_merge",
    "hash_map.from_entries",
    "hash_map.has_key",
];

/// `hash_set.*` op names (all native — serialized-key dedup semantics).
pub const HASH_SET_OPS: &[&str] = &[
    "hash_set.new",
    "hash_set.from",
    "hash_set.to_array",
    "hash_set.len",
    "hash_set.is_empty",
    "hash_set.add",
    "hash_set.remove",
    "hash_set.has",
    "hash_set.union",
    "hash_set.intersection",
    "hash_set.difference",
    "hash_set.unique",
    "hash_set.symmetric_difference",
    "hash_set.without",
    "hash_set.unique_by",
];

/// `array.*` native op names. The arrow-taking members (`take_while`/
/// `drop_while`/`scan`/`foldr`/`find_*`) and the variable-arity members
/// (`push` takes `v...`; `range` takes an optional `step` — arrow params
/// cannot be rest patterns) stay native; the remaining surface is pure
/// handl in `handl-packages/collections/array.hnl`.
pub const ARRAY_OPS: &[&str] = &[
    "array.push",
    "array.range",
    "array.take_while",
    "array.drop_while",
    "array.scan",
    "array.foldr",
    "array.find_index",
    "array.find_last",
    "array.find_last_index",
];

// ---------------------------------------------------------------------------
// Op registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for collections, registering all Rust handls
/// under their full op names (the former `build_registry`).
pub fn collections_op_registry() -> OpRegistry {
    use seq::{End, From, IsEmpty, Len, New, Peek, Pop, Push, ToArray};
    let mut r = OpRegistry::new();

    // vec (back-ended dynamic array)
    r.register("vec.new", Arc::new(New));
    r.register("vec.from", Arc::new(From));
    r.register("vec.to_array", Arc::new(ToArray));
    r.register("vec.len", Arc::new(Len));
    r.register("vec.is_empty", Arc::new(IsEmpty));
    r.register("vec.push", Arc::new(Push { end: End::Back }));
    r.register("vec.pop", Arc::new(Pop { end: End::Back }));
    r.register("vec.peek", Arc::new(Peek { end: End::Back }));

    // list (front-ended list)
    r.register("list.new", Arc::new(New));
    r.register("list.from", Arc::new(From));
    r.register("list.to_array", Arc::new(ToArray));
    r.register("list.len", Arc::new(Len));
    r.register("list.is_empty", Arc::new(IsEmpty));
    r.register("list.push", Arc::new(Push { end: End::Front }));
    r.register("list.pop", Arc::new(Pop { end: End::Front }));
    r.register("list.peek", Arc::new(Peek { end: End::Front }));

    // queue (FIFO)
    r.register("queue.new", Arc::new(New));
    r.register("queue.from", Arc::new(From));
    r.register("queue.to_array", Arc::new(ToArray));
    r.register("queue.len", Arc::new(Len));
    r.register("queue.is_empty", Arc::new(IsEmpty));
    r.register("queue.enqueue", Arc::new(Push { end: End::Back }));
    r.register("queue.dequeue", Arc::new(Pop { end: End::Front }));
    r.register("queue.peek", Arc::new(Peek { end: End::Front }));

    // stack (LIFO)
    r.register("stack.new", Arc::new(New));
    r.register("stack.from", Arc::new(From));
    r.register("stack.to_array", Arc::new(ToArray));
    r.register("stack.len", Arc::new(Len));
    r.register("stack.is_empty", Arc::new(IsEmpty));
    r.register("stack.push", Arc::new(Push { end: End::Back }));
    r.register("stack.pop", Arc::new(Pop { end: End::Back }));
    r.register("stack.peek", Arc::new(Peek { end: End::Back }));

    // deque (double-ended)
    r.register("deque.new", Arc::new(New));
    r.register("deque.from", Arc::new(From));
    r.register("deque.to_array", Arc::new(ToArray));
    r.register("deque.len", Arc::new(Len));
    r.register("deque.is_empty", Arc::new(IsEmpty));
    r.register("deque.push_back", Arc::new(Push { end: End::Back }));
    r.register("deque.push_front", Arc::new(Push { end: End::Front }));
    r.register("deque.pop_back", Arc::new(Pop { end: End::Back }));
    r.register("deque.pop_front", Arc::new(Pop { end: End::Front }));
    r.register("deque.peek_back", Arc::new(Peek { end: End::Back }));
    r.register("deque.peek_front", Arc::new(Peek { end: End::Front }));

    // hash_map
    use hash_map as hm;
    r.register("hash_map.new", Arc::new(hm::New));
    r.register("hash_map.from", Arc::new(hm::From));
    r.register("hash_map.to_object", Arc::new(hm::ToObject));
    r.register("hash_map.len", Arc::new(hm::Len));
    r.register("hash_map.is_empty", Arc::new(hm::IsEmpty));
    r.register("hash_map.get", Arc::new(hm::Get));
    r.register("hash_map.set", Arc::new(hm::Set));
    r.register("hash_map.remove", Arc::new(hm::Remove));
    r.register("hash_map.has", Arc::new(hm::Has));
    r.register("hash_map.keys", Arc::new(hm::Keys));
    r.register("hash_map.values", Arc::new(hm::Values));
    r.register("hash_map.entries", Arc::new(hm::Entries));
    r.register("hash_map.size", Arc::new(hm::Size));
    r.register("hash_map.get_or", Arc::new(hm::GetOr));
    r.register("hash_map.deep_get", Arc::new(hm::DeepGet));
    r.register("hash_map.path_or", Arc::new(hm::PathOr));
    r.register("hash_map.update", Arc::new(hm::Update));
    r.register("hash_map.set_path", Arc::new(hm::SetPath));
    r.register("hash_map.update_path", Arc::new(hm::UpdatePath));
    r.register("hash_map.map_values", Arc::new(hm::MapValues));
    r.register("hash_map.map_keys", Arc::new(hm::MapKeys));
    r.register("hash_map.filter_keys", Arc::new(hm::FilterKeys));
    r.register("hash_map.filter_values", Arc::new(hm::FilterValues));
    r.register("hash_map.invert", Arc::new(hm::Invert));
    r.register("hash_map.clone_deep", Arc::new(hm::CloneDeep));
    r.register("hash_map.pick", Arc::new(hm::Pick));
    r.register("hash_map.omit", Arc::new(hm::Omit));
    r.register("hash_map.merge", Arc::new(hm::Merge));
    r.register("hash_map.deep_merge", Arc::new(hm::DeepMerge));
    r.register("hash_map.from_entries", Arc::new(hm::FromEntries));
    r.register("hash_map.has_key", Arc::new(hm::Has));

    // hash_set
    use hash_set as hs;
    r.register("hash_set.new", Arc::new(hs::New));
    r.register("hash_set.from", Arc::new(hs::From));
    r.register("hash_set.to_array", Arc::new(hs::ToArray));
    r.register("hash_set.len", Arc::new(hs::Len));
    r.register("hash_set.is_empty", Arc::new(hs::IsEmpty));
    r.register("hash_set.add", Arc::new(hs::Add));
    r.register("hash_set.remove", Arc::new(hs::Remove));
    r.register("hash_set.has", Arc::new(hs::Has));
    r.register("hash_set.union", Arc::new(hs::Union));
    r.register("hash_set.intersection", Arc::new(hs::Intersection));
    r.register("hash_set.difference", Arc::new(hs::Difference));
    r.register("hash_set.unique", Arc::new(hs::Unique));
    r.register(
        "hash_set.symmetric_difference",
        Arc::new(hs::SymmetricDifference),
    );
    r.register("hash_set.without", Arc::new(hs::Without));
    r.register("hash_set.unique_by", Arc::new(hs::UniqueBy));

    // array — only the arrow-taking and variable-arity ops stay native
    // (the rest are pure handl in `handl-packages/collections/array.hnl`).
    r.register("array.push", Arc::new(array::Push));
    r.register("array.range", Arc::new(array::Range));
    r.register("array.take_while", Arc::new(array::TakeWhile));
    r.register("array.drop_while", Arc::new(array::DropWhile));
    r.register("array.scan", Arc::new(array::Scan));
    r.register("array.foldr", Arc::new(array::Foldr));
    r.register("array.find_index", Arc::new(array::FindIndex));
    r.register("array.find_last", Arc::new(array::FindLast));
    r.register("array.find_last_index", Arc::new(array::FindLastIndex));

    r
}

// ---------------------------------------------------------------------------
// CollectionsModule — the aggregate module (former module.rs shape)
// ---------------------------------------------------------------------------

/// The `collections` aggregate module — preserves the former
/// `plugin-lib-collections::CollectionsModule` lookup shape.
///
/// The collections module dispatches functions across multiple sub-module
/// prefixes (vec, list, queue, stack, deque, hash_map, hash_set, array).
/// [`Module::function`] splits a qualified name like `"vec.new"` on the
/// first dot and delegates to the corresponding table-built
/// [`HandlModule`]. Because `ModuleRegistry::lookup` first tries every
/// module with the full qualified name, registering this single aggregate
/// resolves both `vec.new` (full-name dispatch) and `collections.vec.new`
/// (module-name + `vec.new` fallback), exactly as before.
pub struct CollectionsModule {
    vec: HandlModule,
    list: HandlModule,
    queue: HandlModule,
    stack: HandlModule,
    deque: HandlModule,
    hash_map: HandlModule,
    hash_set: HandlModule,
    array: HandlModule,
}

impl CollectionsModule {
    /// Build the aggregate from the eight sub-module specs, resolving
    /// declared deps through `resolver`.
    pub fn new(resolver: &dyn ModuleResolver) -> Self {
        use crate::packages::{build_module, module_spec};
        let build = |name: &str| {
            build_module(
                module_spec(name).unwrap_or_else(|| panic!("unknown module '{name}'")),
                resolver,
            )
        };
        Self {
            vec: build("vec"),
            list: build("list"),
            queue: build("queue"),
            stack: build("stack"),
            deque: build("deque"),
            hash_map: build("hash_map"),
            hash_set: build("hash_set"),
            array: build("array"),
        }
    }
}

impl Module for CollectionsModule {
    fn name(&self) -> &str {
        "collections"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        let (prefix, local) = name.split_once('.')?;
        match prefix {
            "vec" => self.vec.function(local),
            "list" => self.list.function(local),
            "queue" => self.queue.function(local),
            "stack" => self.stack.function(local),
            "deque" => self.deque.function(local),
            "hash_map" => self.hash_map.function(local),
            "hash_set" => self.hash_set.function(local),
            "array" => self.array.function(local),
            _ => None,
        }
    }
}
