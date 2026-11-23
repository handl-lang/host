//! Vendors the `lib-type-handl` package into `OUT_DIR` (ADR-0160 P3).
fn main() {
    handl_pkg_vendor::vendor("../../../lib-type-handl").expect("vendor lib-type-handl");
}
