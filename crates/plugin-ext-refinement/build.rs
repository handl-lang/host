//! Vendors the `lib-collections` package into `OUT_DIR` (ADR-0160 P3) — the
//! same mechanism `plugin-lib-stdlib`'s build.rs uses. The arrow-loader
//! tests walk the vendored `src/` tree for `.hnl` arrow files.
fn main() {
    handl_pkg_vendor::vendor("../../../lib-collections")
        .unwrap_or_else(|e| panic!("vendor lib-collections: {e}"));
    // T-044: `tests/str_contract.rs` embeds the `char` contract source —
    // previously reached through `handl-plugin-lib-char-native::CHAR_HNL`;
    // vendor the package directly instead of depending on the crate.
    handl_pkg_vendor::vendor("../../../lib-char")
        .unwrap_or_else(|e| panic!("vendor lib-char: {e}"));
    // Same for `str`: the contract moved out of `handl-packages/str/` into
    // the `lib-str` repo — vendor it so the test stays hermetic.
    handl_pkg_vendor::vendor("../../../lib-str")
        .unwrap_or_else(|e| panic!("vendor lib-str: {e}"));
}
