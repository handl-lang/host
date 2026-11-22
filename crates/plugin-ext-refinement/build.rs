//! Vendors the `lib-collections` package into `OUT_DIR` (ADR-0160 P3) — the
//! same mechanism `plugin-lib-stdlib`'s build.rs uses. The arrow-loader
//! tests walk the vendored `src/` tree for `.hnl` arrow files.
fn main() {
    handl_pkg_vendor::vendor("../../../lib-collections")
        .unwrap_or_else(|e| panic!("vendor lib-collections: {e}"));
}
