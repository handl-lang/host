//! Vendors the const-only `lib-*` packages this crate embeds at build time
//! (ADR-0160 P3) — the same mechanism `plugin-lib-tag`'s build.rs uses. Each
//! call copies `pkg.hnl` + `src/**/*.hnl` into `OUT_DIR/<package>`; `lib.rs`
//! then embeds the vendored files with `include_str!`.
fn main() {
    for pkg in [
        "lib-base64",
        "lib-hex",
        "lib-ip",
        "lib-json",
        "lib-perlin",
        "lib-serialize",
        "lib-simplex",
        "lib-tar",
        "lib-url-rfc",
        "lib-url-whatwg",
    ] {
        handl_pkg_vendor::vendor(format!("../../../{pkg}"))
            .unwrap_or_else(|e| panic!("vendor {pkg}: {e}"));
    }
}
