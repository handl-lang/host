//! The `.hnl` sources of the `str` library (pure handl over `char`).
//!
//! Vendored from the `lib-str-handl` package by this crate's `build.rs`
//! (`handl-pkg-vendor`, ADR-0160 P3) and embedded via `include_str!` — the
//! const-only `handl-lib-str-handl` crate is retired. Re-exported at the
//! crate root so consumers (`kit-prelude`, `examples/loader_bench.rs`)
//! keep resolving `handl_plugin_lib_handl::STR_HANDL_*`.

#![allow(clippy::type_complexity)]

/// The aggregator index (`str` re-exporting every arrow).
pub const STR_HANDL_HNL: &str =
    include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/index.hnl"));

/// The record field names exposed by `str-handl.hnl`.
pub const STR_HANDL_FUNCS: &[&str] = &[
    "upper",
    "lower",
    "trim",
    "trim_start",
    "trim_end",
    "trim_matches",
    "reverse",
    "length",
    "concat",
    "is_empty",
    "equals",
    "is_alpha",
    "is_numeric",
    "is_alphanumeric",
    "capitalize",
    "title_case",
    "snake_case",
    "camel_case",
    "pascal_case",
    "kebab_case",
    "contains",
    "starts_with",
    "ends_with",
    "count",
    "find",
    "rfind",
    "levenshtein",
    "similarity",
    "split",
    "join",
    "replace",
    "replace_first",
    "repeat",
    "char_at",
    "slice",
    "pad_left",
    "pad_right",
    "strip_prefix",
    "strip_suffix",
    "lines",
    "split_at",
    "split_whitespace",
    "truncate",
    "slugify",
    "word_wrap",
];

/// Every arrow file as `(basename, source, exported field names)`, in
/// dependency order (helpers before the invokers that reach them).
pub const STR_HANDL_SOURCES: &[(&str, &str, &[&str])] = &[
    // -- internal/ generic list helpers (no `char`) -------------------------
    (
        "rev",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/rev.hnl"
        )),
        &["rev"],
    ),
    (
        "len",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/len.hnl"
        )),
        &["len"],
    ),
    (
        "head",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/head.hnl"
        )),
        &["head"],
    ),
    (
        "drop",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/drop.hnl"
        )),
        &["drop"],
    ),
    (
        "member",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/member.hnl"
        )),
        &["member"],
    ),
    (
        "prefix_match",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/prefix_match.hnl"
        )),
        &["prefix_match"],
    ),
    (
        "take",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/take.hnl"
        )),
        &["take"],
    ),
    (
        "append",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/append.hnl"
        )),
        &["append"],
    ),
    (
        "last",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/last.hnl"
        )),
        &["last"],
    ),
    (
        "find_idx",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/find_idx.hnl"
        )),
        &["find_idx"],
    ),
    // -- internal/ string helpers (over `char`) -----------------------------
    (
        "capitalize_first",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/capitalize_first.hnl"
        )),
        &["capitalize_first"],
    ),
    (
        "pad_to",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/pad_to.hnl"
        )),
        &["pad_to"],
    ),
    // -- internal/ callback-invoking helpers --------------------------------
    // `map`/`all`/`drop_while` evaluate caller-supplied arrows, whose
    // `char.*`/helper calls resolve in THESE modules' runtimes — they must
    // come after every other internal row (see `STR_HANDL_CALLBACK_SCOPE`).
    (
        "map",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/map.hnl"
        )),
        &["map"],
    ),
    (
        "all",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/all.hnl"
        )),
        &["all"],
    ),
    (
        "drop_while",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/drop_while.hnl"
        )),
        &["drop_while"],
    ),
    (
        "split_words",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/internal/split_words.hnl"
        )),
        &["split_words"],
    ),
    // -- `str` contract arrows (one file per arrow) -------------------------
    (
        "join",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/join.hnl")),
        &["join"],
    ),
    (
        "concat",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/concat.hnl")),
        &["concat"],
    ),
    (
        "is_empty",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/is_empty.hnl")),
        &["is_empty"],
    ),
    (
        "equals",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/equals.hnl")),
        &["equals"],
    ),
    (
        "repeat",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/repeat.hnl")),
        &["repeat"],
    ),
    (
        "upper",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/upper.hnl")),
        &["upper"],
    ),
    (
        "lower",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/lower.hnl")),
        &["lower"],
    ),
    (
        "trim",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/trim.hnl")),
        &["trim"],
    ),
    (
        "trim_start",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/trim_start.hnl"
        )),
        &["trim_start"],
    ),
    (
        "trim_end",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/trim_end.hnl")),
        &["trim_end"],
    ),
    (
        "trim_matches",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/trim_matches.hnl"
        )),
        &["trim_matches"],
    ),
    (
        "reverse",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/reverse.hnl")),
        &["reverse"],
    ),
    (
        "length",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/length.hnl")),
        &["length"],
    ),
    (
        "char_at",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/char_at.hnl")),
        &["char_at"],
    ),
    (
        "slice",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/slice.hnl")),
        &["slice"],
    ),
    (
        "split_at",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/split_at.hnl")),
        &["split_at"],
    ),
    (
        "is_alpha",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/is_alpha.hnl")),
        &["is_alpha"],
    ),
    (
        "is_numeric",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/is_numeric.hnl"
        )),
        &["is_numeric"],
    ),
    (
        "is_alphanumeric",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/is_alphanumeric.hnl"
        )),
        &["is_alphanumeric"],
    ),
    (
        "capitalize",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/capitalize.hnl"
        )),
        &["capitalize"],
    ),
    (
        "title_case",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/title_case.hnl"
        )),
        &["title_case"],
    ),
    (
        "snake_case",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/snake_case.hnl"
        )),
        &["snake_case"],
    ),
    (
        "camel_case",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/camel_case.hnl"
        )),
        &["camel_case"],
    ),
    (
        "pascal_case",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/pascal_case.hnl"
        )),
        &["pascal_case"],
    ),
    (
        "kebab_case",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/kebab_case.hnl"
        )),
        &["kebab_case"],
    ),
    (
        "contains",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/contains.hnl")),
        &["contains"],
    ),
    (
        "starts_with",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/starts_with.hnl"
        )),
        &["starts_with"],
    ),
    (
        "ends_with",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/ends_with.hnl")),
        &["ends_with"],
    ),
    (
        "count",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/count.hnl")),
        &["count"],
    ),
    (
        "find",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/find.hnl")),
        &["find"],
    ),
    (
        "rfind",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/rfind.hnl")),
        &["rfind"],
    ),
    (
        "levenshtein",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/levenshtein.hnl"
        )),
        &["levenshtein"],
    ),
    (
        "similarity",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/similarity.hnl"
        )),
        &["similarity"],
    ),
    (
        "split",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/split.hnl")),
        &["split"],
    ),
    (
        "replace",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/replace.hnl")),
        &["replace"],
    ),
    (
        "replace_first",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/replace_first.hnl"
        )),
        &["replace_first"],
    ),
    (
        "pad_left",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/pad_left.hnl")),
        &["pad_left"],
    ),
    (
        "pad_right",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/pad_right.hnl")),
        &["pad_right"],
    ),
    (
        "strip_prefix",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/strip_prefix.hnl"
        )),
        &["strip_prefix"],
    ),
    (
        "strip_suffix",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/strip_suffix.hnl"
        )),
        &["strip_suffix"],
    ),
    (
        "lines",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/lines.hnl")),
        &["lines"],
    ),
    (
        "split_whitespace",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-str-handl/src/split_whitespace.hnl"
        )),
        &["split_whitespace"],
    ),
    (
        "truncate",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/truncate.hnl")),
        &["truncate"],
    ),
    (
        "slugify",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/slugify.hnl")),
        &["slugify"],
    ),
    (
        "word_wrap",
        include_str!(concat!(env!("OUT_DIR"), "/lib-str-handl/src/word_wrap.hnl")),
        &["word_wrap"],
    ),
];
