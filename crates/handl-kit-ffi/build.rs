//! Vendors the `lib-ffi` package into `OUT_DIR` (ADR-0160 P3): the package
//! repo ships no `Cargo.toml`, and this crate must stay hermetic.
fn main() {
    handl_pkg_vendor::vendor("../../../lib-ffi").expect("vendor lib-ffi");
}
