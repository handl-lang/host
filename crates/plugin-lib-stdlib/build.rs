//! Vendors the lib-* packages this table reads at build time (ADR-0160 P3).
//!
//! Each call copies `pkg.hnl` + `src/**/*.hnl` into `OUT_DIR/<package>` and
//! normalises a legacy `src/<module>.hnl` to `src/index.hnl`, so the table can
//! use one `include_str!` shape per row.
fn main() {
    for pkg in [
        "lib-stats",
        "lib-archive",
        "lib-arrow-iter",
        "lib-base64",
        "lib-bytes",
        "lib-char",
        "lib-cmp",
        "lib-comb",
        "lib-error",
        "lib-collections",
        "lib-crypto",
        "lib-datetime",
        "lib-encoding",
        "lib-hello",
        "lib-hex",
        "lib-intl",
        "lib-intl-c",
        "lib-ip",
        "lib-json",
        "lib-kv-map",
        "lib-mood",
        "lib-option",
        "lib-otlp",
        "lib-obj",
        "lib-parse",
        "lib-rand-c",
        "lib-result",
        "lib-serialize",
        "lib-serialize-toml-c",
        "lib-serialize-yaml-c",
        "lib-tar",
        "lib-tag",
        "lib-posix-c",
        "lib-proc",
        "lib-transform",
        "lib-type",
        "lib-unicode",
        "lib-unicode-c",
        "lib-url-rfc",
        "lib-url-whatwg",
        "lib-utils",
    ] {
        handl_pkg_vendor::vendor(format!("../../../{pkg}"))
            .unwrap_or_else(|e| panic!("vendor {pkg}: {e}"));
    }
}
