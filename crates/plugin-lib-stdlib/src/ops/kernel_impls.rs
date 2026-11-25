//! `impl:` tables for the spec-driven (`RegistrySource::KernelAtoms`)
//! modules whose atoms are kernel [`handl_ops::Op`]s living in the
//! `handl-lib-*`/`lib-*` sibling crates (P-013 T-151).
//!
//! Unlike `ops::math`/`ops::process` (host `Function` impls local to this
//! crate), these modules' computations live in external crates that speak
//! the kernel op ABI — the impl structs are `pub` there, so the tables
//! live here rather than forcing edits into ten sibling repos. Each
//! `*_impls()` returns the `(impl_key → Op)` rows the module's
//! `atoms.yaml` `impl:` fields resolve against; the spec + table fully
//! determine the module's op registry via
//! [`crate::atoms::registry_from_atoms_kernel`].
//!
//! The `*_op_registry()`/`*_OPS` hand-wiring in the sibling crates stays
//! (other consumers still use it); this crate simply stops calling it.

use std::sync::Arc;

use crate::atoms::KernelAtomImpl;

/// `lib-bytes/atoms.yaml` (`module: bytes`) — 8 `bytes.*` atoms.
pub fn bytes_impls() -> Vec<KernelAtomImpl> {
    vec![
        ("bytes_from_list", Arc::new(handl_lib_bytes::ops::FromList)),
        ("bytes_to_list", Arc::new(handl_lib_bytes::ops::ToList)),
        ("bytes_concat", Arc::new(handl_lib_bytes::ops::Concat)),
        ("bytes_slice", Arc::new(handl_lib_bytes::ops::Slice)),
        ("bytes_len", Arc::new(handl_lib_bytes::ops::Len)),
        ("bytes_at", Arc::new(handl_lib_bytes::ops::At)),
        ("bytes_utf8_encode", Arc::new(handl_lib_bytes::ops::Utf8Encode)),
        ("bytes_utf8_decode", Arc::new(handl_lib_bytes::ops::Utf8Decode)),
    ]
}

/// `lib-comb/atoms.yaml` (`module: combinators`) — 16 `combinators.*`
/// atoms; the `fn_*` impl keys come from the `handle ... with rust_fn_*`
/// hints in `combinators.hnl`.
pub fn combinators_impls() -> Vec<KernelAtomImpl> {
    vec![
        ("fn_pipe", Arc::new(handl_lib_comb::ops::Pipe)),
        ("fn_fanout", Arc::new(handl_lib_comb::ops::Fanout)),
        ("fn_choice", Arc::new(handl_lib_comb::ops::Choice)),
        ("fn_apply", Arc::new(handl_lib_comb::ops::Apply)),
        ("fn_flat_map", Arc::new(handl_lib_comb::ops::FlatMap)),
        ("fn_find", Arc::new(handl_lib_comb::ops::Find)),
        ("fn_some", Arc::new(handl_lib_comb::ops::SomeFn)),
        ("fn_every", Arc::new(handl_lib_comb::ops::Every)),
        ("fn_group_by", Arc::new(handl_lib_comb::ops::GroupBy)),
        ("fn_partition", Arc::new(handl_lib_comb::ops::Partition)),
        ("fn_count_by", Arc::new(handl_lib_comb::ops::CountBy)),
        ("fn_tap", Arc::new(handl_lib_comb::ops::Tap)),
        ("fn_match", Arc::new(handl_lib_comb::ops::Match)),
        ("fn_when", Arc::new(handl_lib_comb::ops::When)),
        ("fn_unless", Arc::new(handl_lib_comb::ops::Unless)),
        ("fn_converge", Arc::new(handl_lib_comb::ops::Converge)),
    ]
}

/// `lib-cmp/atoms.yaml` (`module: cmp`) — 20 `cmp.*` atoms.
pub fn cmp_impls() -> Vec<KernelAtomImpl> {
    vec![
        ("cmp_eq", Arc::new(handl_lib_cmp::ops::Eq)),
        ("cmp_ne", Arc::new(handl_lib_cmp::ops::Ne)),
        ("cmp_lt", Arc::new(handl_lib_cmp::ops::Lt)),
        ("cmp_le", Arc::new(handl_lib_cmp::ops::Le)),
        ("cmp_gt", Arc::new(handl_lib_cmp::ops::Gt)),
        ("cmp_ge", Arc::new(handl_lib_cmp::ops::Ge)),
        ("cmp_compare", Arc::new(handl_lib_cmp::ops::Compare)),
        ("cmp_min", Arc::new(handl_lib_cmp::ops::Min)),
        ("cmp_max", Arc::new(handl_lib_cmp::ops::Max)),
        ("cmp_clamp", Arc::new(handl_lib_cmp::ops::Clamp)),
        ("cmp_between", Arc::new(handl_lib_cmp::ops::Between)),
        ("cmp_in_range", Arc::new(handl_lib_cmp::ops::InRange)),
        ("cmp_is_sorted", Arc::new(handl_lib_cmp::ops::IsSorted)),
        ("cmp_is_sorted_desc", Arc::new(handl_lib_cmp::ops::IsSortedDesc)),
        ("cmp_sort", Arc::new(handl_lib_cmp::ops::Sort)),
        ("cmp_sort_desc", Arc::new(handl_lib_cmp::ops::SortDesc)),
        ("cmp_sort_by", Arc::new(handl_lib_cmp::ops::SortBy)),
        ("cmp_sort_by_desc", Arc::new(handl_lib_cmp::ops::SortByDesc)),
        ("cmp_min_by", Arc::new(handl_lib_cmp::ops::MinBy)),
        ("cmp_max_by", Arc::new(handl_lib_cmp::ops::MaxBy)),
    ]
}

/// `lib-error/atoms.yaml` (`module: error`) — 2 `error.*` atoms.
pub fn error_impls() -> Vec<KernelAtomImpl> {
    vec![
        ("error_raise", Arc::new(handl_lib_error::ops::Raise)),
        ("error_try", Arc::new(handl_lib_error::ops::Try)),
    ]
}

/// `lib-mood/atoms.yaml` (`module: mood`) — 6 `mood.*` atoms.
pub fn mood_impls() -> Vec<KernelAtomImpl> {
    vec![
        ("mood_ind", Arc::new(handl_lib_mood::ops::MoodInd)),
        ("mood_int", Arc::new(handl_lib_mood::ops::MoodInt)),
        ("mood_norm", Arc::new(handl_lib_mood::ops::MoodNorm)),
        ("mood_pot", Arc::new(handl_lib_mood::ops::MoodPot)),
        ("mood_neg", Arc::new(handl_lib_mood::ops::MoodNeg)),
        ("mood_cnd", Arc::new(handl_lib_mood::ops::MoodCnd)),
    ]
}

/// `lib-transform/atoms.yaml` (`module: transform`) — 3 `transform.*`
/// atoms; the `fn_*` impl keys come from the `handle ... with rust_fn_*`
/// hints in `index.hnl`.
pub fn transform_impls() -> Vec<KernelAtomImpl> {
    vec![
        ("fn_map", Arc::new(handl_lib_transform::ops::Map)),
        ("fn_filter", Arc::new(handl_lib_transform::ops::Filter)),
        ("fn_reduce", Arc::new(handl_lib_transform::ops::Reduce)),
    ]
}

/// `lib-serialize-toml-c/atoms.yaml` (`module: toml_c`) — 2 atoms.
pub fn toml_c_impls() -> Vec<KernelAtomImpl> {
    vec![
        (
            "toml_c_serialize",
            Arc::new(handl_plugin_lib_serialize_toml_c::TomlCSerialize),
        ),
        (
            "toml_c_deserialize",
            Arc::new(handl_plugin_lib_serialize_toml_c::TomlCDeserialize),
        ),
    ]
}

/// `lib-serialize-yaml-c/atoms.yaml` (`module: yaml_c`) — 2 atoms.
pub fn yaml_c_impls() -> Vec<KernelAtomImpl> {
    vec![
        (
            "yaml_c_serialize",
            Arc::new(handl_plugin_lib_serialize_yaml_c::YamlCSerialize),
        ),
        (
            "yaml_c_deserialize",
            Arc::new(handl_plugin_lib_serialize_yaml_c::YamlCDeserialize),
        ),
    ]
}

/// `lib-obj/atoms.yaml` (`module: obj`) — 5 `obj.*` atoms; the impl keys
/// are the `obj_keys`/`obj_values`/… names in the spec.
pub fn obj_impls() -> Vec<KernelAtomImpl> {
    vec![
        ("obj_keys", Arc::new(handl_lib_obj::ops::Keys)),
        ("obj_values", Arc::new(handl_lib_obj::ops::Values)),
        ("obj_entries", Arc::new(handl_lib_obj::ops::Entries)),
        ("obj_set", Arc::new(handl_lib_obj::ops::Set)),
        ("obj_from_entries", Arc::new(handl_lib_obj::ops::FromEntries)),
    ]
}

/// `lib-type/atoms.yaml` (`module: typeof`) — 25 `typeof.*` atoms. The
/// tag constants are the `pub static StringTag` values in
/// `handl_lib_type::ops` (each a distinct `&'static str` payload);
/// `typeof.type_of`/`typeof.typeof` share the `TypeOf` impl, matching
/// `type_op_registry`.
pub fn typeof_impls() -> Vec<KernelAtomImpl> {
    vec![
        ("typeof_string_tag", Arc::new(handl_lib_type::ops::STRING_TAG)),
        ("typeof_str_tag", Arc::new(handl_lib_type::ops::STR_TAG)),
        ("typeof_number_tag", Arc::new(handl_lib_type::ops::NUMBER_TAG)),
        ("typeof_num_tag", Arc::new(handl_lib_type::ops::NUM_TAG)),
        ("typeof_bool_tag", Arc::new(handl_lib_type::ops::BOOL_TAG)),
        ("typeof_null_tag", Arc::new(handl_lib_type::ops::NULL_TAG)),
        ("typeof_array_tag", Arc::new(handl_lib_type::ops::ARRAY_TAG)),
        ("typeof_arr_tag", Arc::new(handl_lib_type::ops::ARR_TAG)),
        ("typeof_object_tag", Arc::new(handl_lib_type::ops::OBJECT_TAG)),
        ("typeof_obj_tag", Arc::new(handl_lib_type::ops::OBJ_TAG)),
        ("typeof_any_tag", Arc::new(handl_lib_type::ops::ANY_TAG)),
        ("typeof_unknown_tag", Arc::new(handl_lib_type::ops::UNKNOWN_TAG)),
        ("typeof_is", Arc::new(handl_lib_type::ops::TypeIs)),
        ("typeof_must", Arc::new(handl_lib_type::ops::TypeMust)),
        ("typeof_may", Arc::new(handl_lib_type::ops::TypeMay)),
        ("typeof_not", Arc::new(handl_lib_type::ops::TypeNot)),
        ("typeof_when", Arc::new(handl_lib_type::ops::TypeWhen)),
        ("typeof_to_string", Arc::new(handl_lib_type::ops::ToString)),
        ("typeof_to_number", Arc::new(handl_lib_type::ops::ToNumber)),
        ("typeof_to_bool", Arc::new(handl_lib_type::ops::ToBool)),
        ("typeof_type_of", Arc::new(handl_lib_type::ops::TypeOf)),
        ("typeof_identity", Arc::new(handl_lib_type::ops::Identity)),
        ("typeof_default", Arc::new(handl_lib_type::ops::Default)),
        ("typeof_coalesce", Arc::new(handl_lib_type::ops::Coalesce)),
    ]
}

/// `otlp_c.*` — the OTLP exporter ops are *stateful* (all nine share one
/// `Arc<dyn OtlpState>`), so unlike the stateless tables above this one
/// is built by `otlp_op_impls(state)` in lib-otlp-c rather than being a
/// static `fn() -> Vec<_>`. The `atoms.yaml` `impl:` keys are the
/// qualified op names themselves, so this table is a straight pass-
/// through; the state is `HttpOtlpState`, matching what
/// `RegistrySource::Kernel(otlp_std_op_registry)` built before T-151.
pub fn otlp_c_impls() -> Vec<KernelAtomImpl> {
    let state: Arc<dyn handl_plugin_lib_otlp_c::OtlpState> =
        Arc::new(handl_plugin_lib_otlp_std::HttpOtlpState::new());
    handl_plugin_lib_otlp_c::otlp_op_impls(state)
}
