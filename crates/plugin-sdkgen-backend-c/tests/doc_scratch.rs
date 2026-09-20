use handl_plugin_sdkgen::SdkgenBackend;
use handl_plugin_sdkgen_backend_c::CBackend;

#[test]
fn plain_block_comment() {
    let dir = std::env::temp_dir();
    let path = dir.join("scratch_doc.h");
    std::fs::write(
        &path,
        "/* Compresses data. */\nint compress_it(int x);\n\
         /** Doxygen-style. */\nint doxy_it(int x);\n\
         // line comment\nint line_it(int x);\n",
    )
    .unwrap();
    let backend = CBackend::new(None, Default::default(), None, None).unwrap();
    let module = backend.parse_header(&path).expect("parse");
    for f in &module.functions {
        eprintln!("{}: doc = {:?}", f.name, f.doc);
    }
    eprintln!("diags: {:?}", module.diagnostics);
}
