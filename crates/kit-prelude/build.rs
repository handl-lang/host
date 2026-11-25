//! Vendors the `lib-comb` package into `OUT_DIR` (ADR-0160 P3 / T-044) —
//! the same mechanism `handl-kit-stdlib`'s build.rs uses. `lib.rs` embeds
//! the per-arrow `.hnl` files with `include_str!`; the `handl-lib-comb`
//! crate dep is gone.
fn main() {
    handl_pkg_vendor::vendor("../../../lib-comb")
        .unwrap_or_else(|e| panic!("vendor lib-comb: {e}"));
}
