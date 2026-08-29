use handl_plugin_vm::Function;
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::NonFunctionValue;

use super::array::{
    At, Chunk, Compact, Concat, Contains, Dedupe, DeepFlatten, Drop, DropWhile, Enumerate, Fill,
    FindIndex, FindLast, FindLastIndex, First, Flatten, Foldr, IndexOf, InsertAt, Intercalate,
    Intersperse, Last, Length, Pluck, Pop as ArrayPop, Prepend, Push as ArrayPush, Range, RemoveAt,
    Reverse, Scan, Shift as ArrayShift, Slice, Take, TakeWhile, Transpose, Unzip, Windows, Zip,
};
use super::hash_map as hm;
use super::hash_set as hs;
use super::ops::{End, From, IsEmpty, Len, New, Peek, Pop, Push, ToArray};

static NEW: New = New;
static FROM: From = From;
static TO_ARRAY: ToArray = ToArray;
static LEN: Len = Len;
static IS_EMPTY: IsEmpty = IsEmpty;

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

static HASH_MAP_NEW: hm::New = hm::New;
static HASH_MAP_FROM: hm::From = hm::From;
static HASH_MAP_TO_OBJECT: hm::ToObject = hm::ToObject;
static HASH_MAP_LEN: hm::Len = hm::Len;
static HASH_MAP_IS_EMPTY: hm::IsEmpty = hm::IsEmpty;
static HASH_MAP_GET: hm::Get = hm::Get;
static HASH_MAP_SET: hm::Set = hm::Set;
static HASH_MAP_REMOVE: hm::Remove = hm::Remove;
static HASH_MAP_HAS: hm::Has = hm::Has;
static HASH_MAP_KEYS: hm::Keys = hm::Keys;
static HASH_MAP_VALUES: hm::Values = hm::Values;

static HASH_MAP_ENTRIES: hm::Entries = hm::Entries;
static HASH_MAP_SIZE: hm::Size = hm::Size;
static HASH_MAP_GET_OR: hm::GetOr = hm::GetOr;
static HASH_MAP_DEEP_GET: hm::DeepGet = hm::DeepGet;
static HASH_MAP_PATH_OR: hm::PathOr = hm::PathOr;
static HASH_MAP_UPDATE: hm::Update = hm::Update;
static HASH_MAP_SET_PATH: hm::SetPath = hm::SetPath;
static HASH_MAP_UPDATE_PATH: hm::UpdatePath = hm::UpdatePath;
static HASH_MAP_MAP_VALUES: hm::MapValues = hm::MapValues;
static HASH_MAP_MAP_KEYS: hm::MapKeys = hm::MapKeys;
static HASH_MAP_FILTER_KEYS: hm::FilterKeys = hm::FilterKeys;
static HASH_MAP_FILTER_VALUES: hm::FilterValues = hm::FilterValues;
static HASH_MAP_INVERT: hm::Invert = hm::Invert;
static HASH_MAP_CLONE_DEEP: hm::CloneDeep = hm::CloneDeep;
static HASH_MAP_PICK: hm::Pick = hm::Pick;
static HASH_MAP_OMIT: hm::Omit = hm::Omit;
static HASH_MAP_MERGE: hm::Merge = hm::Merge;
static HASH_MAP_DEEP_MERGE: hm::DeepMerge = hm::DeepMerge;
static HASH_MAP_FROM_ENTRIES: hm::FromEntries = hm::FromEntries;
static HASH_MAP_HAS_KEY: hm::Has = hm::Has;

static HASH_SET_NEW: hs::New = hs::New;
static HASH_SET_FROM: hs::From = hs::From;
static HASH_SET_TO_ARRAY: hs::ToArray = hs::ToArray;
static HASH_SET_LEN: hs::Len = hs::Len;
static HASH_SET_IS_EMPTY: hs::IsEmpty = hs::IsEmpty;
static HASH_SET_ADD: hs::Add = hs::Add;
static HASH_SET_REMOVE: hs::Remove = hs::Remove;
static HASH_SET_HAS: hs::Has = hs::Has;
static HASH_SET_UNION: hs::Union = hs::Union;
static HASH_SET_INTERSECTION: hs::Intersection = hs::Intersection;
static HASH_SET_DIFFERENCE: hs::Difference = hs::Difference;
static HASH_SET_UNIQUE: hs::Unique = hs::Unique;
static HASH_SET_SYMMETRIC_DIFFERENCE: hs::SymmetricDifference = hs::SymmetricDifference;
static HASH_SET_WITHOUT: hs::Without = hs::Without;
static HASH_SET_UNIQUE_BY: hs::UniqueBy = hs::UniqueBy;

static ARRAY_LENGTH: Length = Length;
static ARRAY_REVERSE: Reverse = Reverse;
static ARRAY_CONCAT: Concat = Concat;
static ARRAY_CONTAINS: Contains = Contains;
static ARRAY_INDEX_OF: IndexOf = IndexOf;
static ARRAY_SLICE: Slice = Slice;
static ARRAY_FIRST: First = First;
static ARRAY_LAST: Last = Last;
static ARRAY_AT: At = At;
static ARRAY_PUSH: ArrayPush = ArrayPush;
static ARRAY_POP: ArrayPop = ArrayPop;
static ARRAY_SHIFT: ArrayShift = ArrayShift;
static ARRAY_FILL: Fill = Fill;
static ARRAY_FLATTEN: Flatten = Flatten;
static ARRAY_DEEP_FLATTEN: DeepFlatten = DeepFlatten;

static ARRAY_TAKE: Take = Take;
static ARRAY_DROP: Drop = Drop;
static ARRAY_ENUMERATE: Enumerate = Enumerate;
static ARRAY_UNZIP: Unzip = Unzip;
static ARRAY_INTERSPERSE: Intersperse = Intersperse;
static ARRAY_INTERCALATE: Intercalate = Intercalate;
static ARRAY_DEDUPE: Dedupe = Dedupe;
static ARRAY_PREPEND: Prepend = Prepend;
static ARRAY_TAKE_WHILE: TakeWhile = TakeWhile;
static ARRAY_DROP_WHILE: DropWhile = DropWhile;
static ARRAY_SCAN: Scan = Scan;
static ARRAY_FOLDR: Foldr = Foldr;
static ARRAY_FIND_INDEX: FindIndex = FindIndex;
static ARRAY_FIND_LAST: FindLast = FindLast;
static ARRAY_FIND_LAST_INDEX: FindLastIndex = FindLastIndex;
static ARRAY_WINDOWS: Windows = Windows;
static ARRAY_TRANSPOSE: Transpose = Transpose;
static ARRAY_INSERT_AT: InsertAt = InsertAt;
static ARRAY_REMOVE_AT: RemoveAt = RemoveAt;

static ARRAY_RANGE: Range = Range;
static ARRAY_ZIP: Zip = Zip;
static ARRAY_CHUNK: Chunk = Chunk;
static ARRAY_COMPACT: Compact = Compact;
static ARRAY_PLUCK: Pluck = Pluck;

/// The `collections` module — collection types for the handl runtime.
///
/// Currently implemented: `vec`, `list`, `queue`, `stack`, `deque`, `hash_map`, `hash_set`.
#[derive(Clone, Copy)]
pub struct CollectionsModule;

impl Module for CollectionsModule {
    fn name(&self) -> &str {
        "collections"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            // vec
            "vec.new" => Some(&NEW),
            "vec.from" => Some(&FROM),
            "vec.to_array" => Some(&TO_ARRAY),
            "vec.len" => Some(&LEN),
            "vec.is_empty" => Some(&IS_EMPTY),
            "vec.push" => Some(&VEC_PUSH),
            "vec.pop" => Some(&VEC_POP),
            "vec.peek" => Some(&VEC_PEEK),
            // list
            "list.new" => Some(&NEW),
            "list.from" => Some(&FROM),
            "list.to_array" => Some(&TO_ARRAY),
            "list.len" => Some(&LEN),
            "list.is_empty" => Some(&IS_EMPTY),
            "list.push" => Some(&LIST_PUSH),
            "list.pop" => Some(&LIST_POP),
            "list.peek" => Some(&LIST_PEEK),
            // queue
            "queue.new" => Some(&NEW),
            "queue.from" => Some(&FROM),
            "queue.to_array" => Some(&TO_ARRAY),
            "queue.len" => Some(&LEN),
            "queue.is_empty" => Some(&IS_EMPTY),
            "queue.enqueue" => Some(&QUEUE_ENQUEUE),
            "queue.dequeue" => Some(&QUEUE_DEQUEUE),
            "queue.peek" => Some(&QUEUE_PEEK),
            // stack
            "stack.new" => Some(&NEW),
            "stack.from" => Some(&FROM),
            "stack.to_array" => Some(&TO_ARRAY),
            "stack.len" => Some(&LEN),
            "stack.is_empty" => Some(&IS_EMPTY),
            "stack.push" => Some(&STACK_PUSH),
            "stack.pop" => Some(&STACK_POP),
            "stack.peek" => Some(&STACK_PEEK),
            // deque
            "deque.new" => Some(&NEW),
            "deque.from" => Some(&FROM),
            "deque.to_array" => Some(&TO_ARRAY),
            "deque.len" => Some(&LEN),
            "deque.is_empty" => Some(&IS_EMPTY),
            "deque.push_back" => Some(&DEQUE_PUSH_BACK),
            "deque.push_front" => Some(&DEQUE_PUSH_FRONT),
            "deque.pop_back" => Some(&DEQUE_POP_BACK),
            "deque.pop_front" => Some(&DEQUE_POP_FRONT),
            "deque.peek_back" => Some(&DEQUE_PEEK_BACK),
            "deque.peek_front" => Some(&DEQUE_PEEK_FRONT),
            // hash_map
            "hash_map.new" => Some(&HASH_MAP_NEW),
            "hash_map.from" => Some(&HASH_MAP_FROM),
            "hash_map.to_object" => Some(&HASH_MAP_TO_OBJECT),
            "hash_map.len" => Some(&HASH_MAP_LEN),
            "hash_map.is_empty" => Some(&HASH_MAP_IS_EMPTY),
            "hash_map.get" => Some(&HASH_MAP_GET),
            "hash_map.set" => Some(&HASH_MAP_SET),
            "hash_map.remove" => Some(&HASH_MAP_REMOVE),
            "hash_map.has" => Some(&HASH_MAP_HAS),
            "hash_map.keys" => Some(&HASH_MAP_KEYS),
            "hash_map.values" => Some(&HASH_MAP_VALUES),
            "hash_map.entries" => Some(&HASH_MAP_ENTRIES),
            "hash_map.size" => Some(&HASH_MAP_SIZE),
            "hash_map.get_or" => Some(&HASH_MAP_GET_OR),
            "hash_map.deep_get" => Some(&HASH_MAP_DEEP_GET),
            "hash_map.path_or" => Some(&HASH_MAP_PATH_OR),
            "hash_map.update" => Some(&HASH_MAP_UPDATE),
            "hash_map.set_path" => Some(&HASH_MAP_SET_PATH),
            "hash_map.update_path" => Some(&HASH_MAP_UPDATE_PATH),
            "hash_map.map_values" => Some(&HASH_MAP_MAP_VALUES),
            "hash_map.map_keys" => Some(&HASH_MAP_MAP_KEYS),
            "hash_map.filter_keys" => Some(&HASH_MAP_FILTER_KEYS),
            "hash_map.filter_values" => Some(&HASH_MAP_FILTER_VALUES),
            "hash_map.invert" => Some(&HASH_MAP_INVERT),
            "hash_map.clone_deep" => Some(&HASH_MAP_CLONE_DEEP),
            "hash_map.pick" => Some(&HASH_MAP_PICK),
            "hash_map.omit" => Some(&HASH_MAP_OMIT),
            "hash_map.merge" => Some(&HASH_MAP_MERGE),
            "hash_map.deep_merge" => Some(&HASH_MAP_DEEP_MERGE),
            "hash_map.from_entries" => Some(&HASH_MAP_FROM_ENTRIES),
            "hash_map.has_key" => Some(&HASH_MAP_HAS_KEY),
            // hash_set
            "hash_set.new" => Some(&HASH_SET_NEW),
            "hash_set.from" => Some(&HASH_SET_FROM),
            "hash_set.to_array" => Some(&HASH_SET_TO_ARRAY),
            "hash_set.len" => Some(&HASH_SET_LEN),
            "hash_set.is_empty" => Some(&HASH_SET_IS_EMPTY),
            "hash_set.add" => Some(&HASH_SET_ADD),
            "hash_set.remove" => Some(&HASH_SET_REMOVE),
            "hash_set.has" => Some(&HASH_SET_HAS),
            "hash_set.union" => Some(&HASH_SET_UNION),
            "hash_set.intersection" => Some(&HASH_SET_INTERSECTION),
            "hash_set.difference" => Some(&HASH_SET_DIFFERENCE),
            "hash_set.unique" => Some(&HASH_SET_UNIQUE),
            "hash_set.symmetric_difference" => Some(&HASH_SET_SYMMETRIC_DIFFERENCE),
            "hash_set.without" => Some(&HASH_SET_WITHOUT),
            "hash_set.unique_by" => Some(&HASH_SET_UNIQUE_BY),
            // array
            "array.length" => Some(&ARRAY_LENGTH),
            "array.len" => Some(&ARRAY_LENGTH),
            "array.reverse" => Some(&ARRAY_REVERSE),
            "array.concat" => Some(&ARRAY_CONCAT),
            "array.contains" => Some(&ARRAY_CONTAINS),
            "array.index_of" => Some(&ARRAY_INDEX_OF),
            "array.slice" => Some(&ARRAY_SLICE),
            "array.first" => Some(&ARRAY_FIRST),
            "array.last" => Some(&ARRAY_LAST),
            "array.at" => Some(&ARRAY_AT),
            "array.push" => Some(&ARRAY_PUSH),
            "array.pop" => Some(&ARRAY_POP),
            "array.shift" => Some(&ARRAY_SHIFT),
            "array.fill" => Some(&ARRAY_FILL),
            "array.flatten" => Some(&ARRAY_FLATTEN),
            "array.deep_flatten" => Some(&ARRAY_DEEP_FLATTEN),
            "array.take" => Some(&ARRAY_TAKE),
            "array.drop" => Some(&ARRAY_DROP),
            "array.enumerate" => Some(&ARRAY_ENUMERATE),
            "array.unzip" => Some(&ARRAY_UNZIP),
            "array.intersperse" => Some(&ARRAY_INTERSPERSE),
            "array.intercalate" => Some(&ARRAY_INTERCALATE),
            "array.dedupe" => Some(&ARRAY_DEDUPE),
            "array.prepend" => Some(&ARRAY_PREPEND),
            "array.take_while" => Some(&ARRAY_TAKE_WHILE),
            "array.drop_while" => Some(&ARRAY_DROP_WHILE),
            "array.scan" => Some(&ARRAY_SCAN),
            "array.foldr" => Some(&ARRAY_FOLDR),
            "array.find_index" => Some(&ARRAY_FIND_INDEX),
            "array.find_last" => Some(&ARRAY_FIND_LAST),
            "array.find_last_index" => Some(&ARRAY_FIND_LAST_INDEX),
            "array.windows" => Some(&ARRAY_WINDOWS),
            "array.transpose" => Some(&ARRAY_TRANSPOSE),
            "array.insert_at" => Some(&ARRAY_INSERT_AT),
            "array.remove_at" => Some(&ARRAY_REMOVE_AT),
            "array.range" => Some(&ARRAY_RANGE),
            "array.zip" => Some(&ARRAY_ZIP),
            "array.chunk" => Some(&ARRAY_CHUNK),
            "array.compact" => Some(&ARRAY_COMPACT),
            "array.pluck" => Some(&ARRAY_PLUCK),
            _ => None,
        }
    }
}
