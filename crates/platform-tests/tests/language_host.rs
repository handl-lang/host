use std::path::PathBuf;

use handl_platform::{Kit, KitManifest, LanguageHost, SyntaxNode};
use handl_plugin_comments::CommentsPlugin;
use handl_plugin_doc_comments::DocCommentsPlugin;

fn data_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(name)
}

fn genesis_plugins() -> Vec<handl_platform::PluginManifest> {
    vec![
        CommentsPlugin::new().manifest(),
        DocCommentsPlugin::new().manifest(),
    ]
}

#[test]
fn compose_with_comments() {
    let manifest = KitManifest::load(data_path("language_comments.toml")).unwrap();
    let kit = Kit {
        manifest,
        available_plugins: genesis_plugins(),
    };

    let mut host = LanguageHost::new();
    host.register_plugin("comments", CommentsPlugin::register_all);

    let out = host.run_language_pipeline(&kit, "// hello").unwrap();
    assert_eq!(out.formatted, "// hello\n");
    assert!(out.diagnostics.is_empty());
    assert!(out.ast.iter().any(|n| matches!(n, SyntaxNode::LineComment { .. })));
}

#[test]
fn compose_with_doc_comments() {
    let manifest = KitManifest::load(data_path("language_doc.toml")).unwrap();
    let kit = Kit {
        manifest,
        available_plugins: genesis_plugins(),
    };

    let mut host = LanguageHost::new();
    host.register_plugin("comments", CommentsPlugin::register_all);
    host.register_plugin("doc-comments", DocCommentsPlugin::register_all);

    let out = host.run_language_pipeline(&kit, "/// doc").unwrap();
    assert_eq!(out.formatted, "/// doc\n");
    assert!(out.diagnostics.is_empty());
    assert!(out.ast.iter().any(|n| matches!(n, SyntaxNode::DocComment { .. })));
}
