//! Vendors the `lib-desugar`/`lib-loader`/`lib-str-handl` packages into
//! `OUT_DIR` (ADR-0160 P3) — the same mechanism `plugin-lib-stdlib`'s
//! build.rs uses. The const-only `handl-lib-*` crates are retired;
//! `DESUGAR_HNL`/`LOADER_HNL`/`STR_HANDL_*` below embed the vendored
//! `src/*.hnl` sources directly.
fn main() {
    for pkg in ["lib-desugar", "lib-loader", "lib-str-handl"] {
        handl_pkg_vendor::vendor(format!("../../../{pkg}"))
            .unwrap_or_else(|e| panic!("vendor {pkg}: {e}"));
    }
}
