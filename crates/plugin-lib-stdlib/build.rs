//! Vendors the lib-* packages this table reads at build time (ADR-0160 P3).
//!
//! Each call copies `pkg.hnl` + `src/**/*.hnl` into `OUT_DIR/<package>` and
//! normalises a legacy `src/<module>.hnl` to `src/index.hnl`, so the table can
//! use one `include_str!` shape per row.
fn main() {
    for pkg in ["lib-stats"] {
        handl_pkg_vendor::vendor(format!("../../../{pkg}"))
            .unwrap_or_else(|e| panic!("vendor {pkg}: {e}"));
    }
}
