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
        "lib-char",
        "lib-collections",
        "lib-datetime",
        "lib-encoding",
        "lib-hello",
        "lib-hex",
        "lib-intl",
        "lib-intl-c",
        "lib-ip",
        "lib-json",
        "lib-kv-map",
        "lib-option",
        "lib-otlp",
        "lib-parse",
        "lib-rand-c",
        "lib-result",
        "lib-serialize",
        "lib-serialize-toml-c",
        "lib-serialize-yaml-c",
        "lib-tar",
        "lib-time-c",
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
