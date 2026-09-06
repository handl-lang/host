use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use super::array::{
    At, Chunk, Compact, Concat, Contains, Dedupe, DeepFlatten, Drop, DropWhile, Enumerate, Fill,
    FindIndex, FindLast, FindLastIndex, First, Flatten, Foldr, IndexOf, InsertAt, Intercalate,
    Intersperse, Last, Length, Pluck, Pop as ArrayPop, Prepend, Push as ArrayPush, Range, RemoveAt,
    Reverse, Scan, Shift as ArrayShift, Slice, Take, TakeWhile, Transpose, Unzip, Windows, Zip,
};
use super::hash_map as hm;
use super::hash_set as hs;
use super::ops::{End, From, IsEmpty, Len, New, Peek, Pop, Push, ToArray};

/// The handl source declaring the aggregated collections interface — the
/// single source of truth for the collections API surface. Loaded at
/// compile time from the kit-stdlib data dir.
///
/// The `.hnl` file contains `---` doc comments and `cap` declarations for
/// all collections-owned sub-modules. The [`HandlModule`] loader strips
/// comments and declaration lines, leaving an empty expression (no
/// pure-logic arrows); all functions are resolved via [`OpFunction`]
/// dispatching to the [`OpRegistry`].
const HNL: &str = include_str!("../../../handl-packages/collections/collections.hnl");

// ---------------------------------------------------------------------------
// Static instances for stateful ops (Push/Pop/Peek with End variants)
// ---------------------------------------------------------------------------

static VEC_PUSH: Push = Push { end: End::Back };
static VEC_POP: Pop = Pop { end: End::Back };
static VEC_PEEK: Peek = Peek { end: End::Back };

static LIST_PUSH: Push = Push { end: End::Front };
static LIST_POP: Pop = Pop { end: End::Front };
static LIST_PEEK: Peek = Peek { end: End::Front };

static QUEUE_ENQUEUE: Push = Push { end: End::Back };
static QUEUE_DEQUEUE: Pop = Pop { end: End::Front };
static QUEUE_PEEK: Peek = Peek { end: End::Front };

static STACK_PUSH: Push = Push { end: End::Back };
static STACK_POP: Pop = Pop { end: End::Back };
static STACK_PEEK: Peek = Peek { end: End::Back };

static DEQUE_PUSH_BACK: Push = Push { end: End::Back };
static DEQUE_PUSH_FRONT: Push = Push { end: End::Front };
static DEQUE_POP_BACK: Pop = Pop { end: End::Back };
static DEQUE_POP_FRONT: Pop = Pop { end: End::Front };
static DEQUE_PEEK_BACK: Peek = Peek { end: End::Back };
static DEQUE_PEEK_FRONT: Peek = Peek { end: End::Front };

// ---------------------------------------------------------------------------
// Op names per sub-module
// ---------------------------------------------------------------------------

const VEC_OPS: &[&str] = &[
    "vec.new",
    "vec.from",
    "vec.to_array",
    "vec.len",
    "vec.is_empty",
    "vec.push",
    "vec.pop",
    "vec.peek",
];

const LIST_OPS: &[&str] = &[
    "list.new",
    "list.from",
    "list.to_array",
    "list.len",
    "list.is_empty",
    "list.push",
    "list.pop",
    "list.peek",
];

const QUEUE_OPS: &[&str] = &[
    "queue.new",
    "queue.from",
    "queue.to_array",
    "queue.len",
    "queue.is_empty",
    "queue.enqueue",
    "queue.dequeue",
    "queue.peek",
];

const STACK_OPS: &[&str] = &[
    "stack.new",
    "stack.from",
    "stack.to_array",
    "stack.len",
    "stack.is_empty",
    "stack.push",
    "stack.pop",
    "stack.peek",
];

const DEQUE_OPS: &[&str] = &[
    "deque.new",
    "deque.from",
    "deque.to_array",
    "deque.len",
    "deque.is_empty",
    "deque.push_back",
    "deque.push_front",
    "deque.pop_back",
    "deque.pop_front",
    "deque.peek_back",
    "deque.peek_front",
];

const HASH_MAP_OPS: &[&str] = &[
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

const HASH_SET_OPS: &[&str] = &[
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

const ARRAY_OPS: &[&str] = &[
    "array.len",
    "array.reverse",
    "array.concat",
    "array.contains",
    "array.index_of",
    "array.slice",
    "array.first",
    "array.last",
    "array.at",
    "array.push",
    "array.pop",
    "array.shift",
    "array.fill",
    "array.flatten",
    "array.deep_flatten",
    "array.take",
    "array.drop",
    "array.enumerate",
    "array.unzip",
    "array.intersperse",
    "array.intercalate",
    "array.dedupe",
    "array.prepend",
    "array.take_while",
    "array.drop_while",
    "array.scan",
    "array.foldr",
    "array.find_index",
    "array.find_last",
    "array.find_last_index",
    "array.windows",
    "array.transpose",
    "array.insert_at",
    "array.remove_at",
    "array.range",
    "array.zip",
    "array.chunk",
    "array.compact",
    "array.pluck",
];

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for collections, registering all Rust handls
/// under their full op names.
///
/// Each op name (e.g. `"vec.new"`) maps to its Rust `Function` impl. Shared
/// ops (e.g. `New`, `From`, `Len`) are registered under multiple op names
/// (one per sub-module). Stateful ops (`Push`/`Pop`/`Peek` with `End`
/// variants) use dedicated static instances per sub-module.
fn build_registry() -> OpRegistry {
    let mut r = OpRegistry::new();

    // vec (back-ended dynamic array)
    r.register("vec.new", Arc::new(New));
    r.register("vec.from", Arc::new(From));
    r.register("vec.to_array", Arc::new(ToArray));
    r.register("vec.len", Arc::new(Len));
    r.register("vec.is_empty", Arc::new(IsEmpty));
    r.register("vec.push", Arc::new(VEC_PUSH));
    r.register("vec.pop", Arc::new(VEC_POP));
    r.register("vec.peek", Arc::new(VEC_PEEK));

    // list (front-ended list)
    r.register("list.new", Arc::new(New));
    r.register("list.from", Arc::new(From));
    r.register("list.to_array", Arc::new(ToArray));
    r.register("list.len", Arc::new(Len));
    r.register("list.is_empty", Arc::new(IsEmpty));
    r.register("list.push", Arc::new(LIST_PUSH));
    r.register("list.pop", Arc::new(LIST_POP));
    r.register("list.peek", Arc::new(LIST_PEEK));

    // queue (FIFO)
    r.register("queue.new", Arc::new(New));
    r.register("queue.from", Arc::new(From));
    r.register("queue.to_array", Arc::new(ToArray));
    r.register("queue.len", Arc::new(Len));
    r.register("queue.is_empty", Arc::new(IsEmpty));
    r.register("queue.enqueue", Arc::new(QUEUE_ENQUEUE));
    r.register("queue.dequeue", Arc::new(QUEUE_DEQUEUE));
    r.register("queue.peek", Arc::new(QUEUE_PEEK));

    // stack (LIFO)
    r.register("stack.new", Arc::new(New));
    r.register("stack.from", Arc::new(From));
    r.register("stack.to_array", Arc::new(ToArray));
    r.register("stack.len", Arc::new(Len));
    r.register("stack.is_empty", Arc::new(IsEmpty));
    r.register("stack.push", Arc::new(STACK_PUSH));
    r.register("stack.pop", Arc::new(STACK_POP));
    r.register("stack.peek", Arc::new(STACK_PEEK));

    // deque (double-ended)
    r.register("deque.new", Arc::new(New));
    r.register("deque.from", Arc::new(From));
    r.register("deque.to_array", Arc::new(ToArray));
    r.register("deque.len", Arc::new(Len));
    r.register("deque.is_empty", Arc::new(IsEmpty));
    r.register("deque.push_back", Arc::new(DEQUE_PUSH_BACK));
    r.register("deque.push_front", Arc::new(DEQUE_PUSH_FRONT));
    r.register("deque.pop_back", Arc::new(DEQUE_POP_BACK));
    r.register("deque.pop_front", Arc::new(DEQUE_POP_FRONT));
    r.register("deque.peek_back", Arc::new(DEQUE_PEEK_BACK));
    r.register("deque.peek_front", Arc::new(DEQUE_PEEK_FRONT));

    // hash_map
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
    r.register("hash_set.symmetric_difference", Arc::new(hs::SymmetricDifference));
    r.register("hash_set.without", Arc::new(hs::Without));
    r.register("hash_set.unique_by", Arc::new(hs::UniqueBy));

    // array (functional array operations)
    r.register("array.len", Arc::new(Length));
    r.register("array.reverse", Arc::new(Reverse));
    r.register("array.concat", Arc::new(Concat));
    r.register("array.contains", Arc::new(Contains));
    r.register("array.index_of", Arc::new(IndexOf));
    r.register("array.slice", Arc::new(Slice));
    r.register("array.first", Arc::new(First));
    r.register("array.last", Arc::new(Last));
    r.register("array.at", Arc::new(At));
    r.register("array.push", Arc::new(ArrayPush));
    r.register("array.pop", Arc::new(ArrayPop));
    r.register("array.shift", Arc::new(ArrayShift));
    r.register("array.fill", Arc::new(Fill));
    r.register("array.flatten", Arc::new(Flatten));
    r.register("array.deep_flatten", Arc::new(DeepFlatten));
    r.register("array.take", Arc::new(Take));
    r.register("array.drop", Arc::new(Drop));
    r.register("array.enumerate", Arc::new(Enumerate));
    r.register("array.unzip", Arc::new(Unzip));
    r.register("array.intersperse", Arc::new(Intersperse));
    r.register("array.intercalate", Arc::new(Intercalate));
    r.register("array.dedupe", Arc::new(Dedupe));
    r.register("array.prepend", Arc::new(Prepend));
    r.register("array.take_while", Arc::new(TakeWhile));
    r.register("array.drop_while", Arc::new(DropWhile));
    r.register("array.scan", Arc::new(Scan));
    r.register("array.foldr", Arc::new(Foldr));
    r.register("array.find_index", Arc::new(FindIndex));
    r.register("array.find_last", Arc::new(FindLast));
    r.register("array.find_last_index", Arc::new(FindLastIndex));
    r.register("array.windows", Arc::new(Windows));
    r.register("array.transpose", Arc::new(Transpose));
    r.register("array.insert_at", Arc::new(InsertAt));
    r.register("array.remove_at", Arc::new(RemoveAt));
    r.register("array.range", Arc::new(Range));
    r.register("array.zip", Arc::new(Zip));
    r.register("array.chunk", Arc::new(Chunk));
    r.register("array.compact", Arc::new(Compact));
    r.register("array.pluck", Arc::new(Pluck));

    r
}

// ---------------------------------------------------------------------------
// HandlModule sub-modules (one per sub-module prefix)
// ---------------------------------------------------------------------------

/// The 8 [`HandlModule`] instances, one per collections sub-module prefix.
///
/// The collections module dispatches functions across multiple sub-module
/// names (vec, list, queue, stack, deque, hash_map, hash_set, array). Each
/// sub-module's ops have unique local names (the segment after the last
/// `.`), so [`HandlModule::with_ops`] stores them without collision. The
/// shared [`OpRegistry`] is cloned (via [`Arc::clone`]) into each
/// sub-module.
struct CollectionsModules {
    vec: HandlModule,
    list: HandlModule,
    queue: HandlModule,
    stack: HandlModule,
    deque: HandlModule,
    hash_map: HandlModule,
    hash_set: HandlModule,
    array: HandlModule,
}

/// Build the [`CollectionsModules`] from the shared [`OpRegistry`] and the
/// `.hnl` source.
fn build_modules() -> CollectionsModules {
    let registry = Arc::new(build_registry());
    CollectionsModules {
        vec: HandlModule::new("vec", HNL, &[]).with_ops(VEC_OPS, Arc::clone(&registry)),
        list: HandlModule::new("list", HNL, &[]).with_ops(LIST_OPS, Arc::clone(&registry)),
        queue: HandlModule::new("queue", HNL, &[]).with_ops(QUEUE_OPS, Arc::clone(&registry)),
        stack: HandlModule::new("stack", HNL, &[]).with_ops(STACK_OPS, Arc::clone(&registry)),
        deque: HandlModule::new("deque", HNL, &[]).with_ops(DEQUE_OPS, Arc::clone(&registry)),
        hash_map: HandlModule::new("hash_map", HNL, &[])
            .with_ops(HASH_MAP_OPS, Arc::clone(&registry)),
        hash_set: HandlModule::new("hash_set", HNL, &[])
            .with_ops(HASH_SET_OPS, Arc::clone(&registry)),
        array: HandlModule::new("array", HNL, &[]).with_ops(ARRAY_OPS, Arc::clone(&registry)),
    }
}

/// A thread-safe lazy-initialized [`CollectionsModules`].
static MODULES: OnceLock<CollectionsModules> = OnceLock::new();

/// Get the shared [`CollectionsModules`], initializing on first call.
fn modules() -> &'static CollectionsModules {
    MODULES.get_or_init(build_modules)
}

// ---------------------------------------------------------------------------
// CollectionsModule
// ---------------------------------------------------------------------------

/// The `collections` module — collection types for the handl runtime.
///
/// Currently implemented: `vec`, `list`, `queue`, `stack`, `deque`,
/// `hash_map`, `hash_set`, `array`.
///
/// This is an **aggregator** module: it dispatches functions across
/// multiple sub-module prefixes (vec, list, queue, stack, deque, hash_map,
/// hash_set, array). Each sub-module is backed by a [`HandlModule`] with
/// op functions dispatching to a shared [`OpRegistry`]. The interface is
/// declared in `collections.hnl` via `cap` declarations; this struct
/// delegates [`Module`] lookups to the shared [`CollectionsModules`].
#[derive(Clone, Copy)]
pub struct CollectionsModule;

impl Module for CollectionsModule {
    fn name(&self) -> &str {
        "collections"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        // The collections module dispatches on dotted function names like
        // "vec.new", "hash_map.get", etc. Split on the first dot to get the
        // sub-module prefix and the local function name, then delegate to
        // the appropriate HandlModule.
        let (prefix, local) = name.split_once('.')?;
        let m = modules();
        match prefix {
            "vec" => m.vec.function(local),
            "list" => m.list.function(local),
            "queue" => m.queue.function(local),
            "stack" => m.stack.function(local),
            "deque" => m.deque.function(local),
            "hash_map" => m.hash_map.function(local),
            "hash_set" => m.hash_set.function(local),
            "array" => m.array.function(local),
            _ => None,
        }
    }
}
