//! Vendors the `lib-tag` package into `OUT_DIR` (ADR-0160 P3) — the same
//! mechanism `plugin-lib-stdlib`'s build.rs uses. The const-only
//! `handl-lib-tag` crate is retired; `TAG_HNL` below embeds the vendored
//! `src/index.hnl` directly.
fn main() {
    handl_pkg_vendor::vendor("../../../lib-tag")
        .unwrap_or_else(|e| panic!("vendor lib-tag: {e}"));
}
