use handl_host_tooling::lsp::CompletionRequest;
use handl_kit_data_canonical::language_from_yaml;
use handl_plugin_lsp::LspEngine;
use std::io::Write;

#[test]
fn import_name_completion_lists_exports() {
    let lang = language_from_yaml().expect("canonical language assembles");
    let engine = LspEngine::new(&lang);

    let tmp = std::env::temp_dir().join("handl-plugin-lsp-test-import-name");
    std::fs::remove_dir_all(&tmp).ok();
    std::fs::create_dir_all(&tmp).unwrap();

    let target = tmp.join("mymod.hnl");
    let mut f = std::fs::File::create(&target).unwrap();
    write!(f, "square = 1\n").unwrap();

    let source = "from \"mymod\" import square".to_string();
    let pos = source.find("square").unwrap() - 1;
    let request = CompletionRequest {
        source: source.clone(),
        pos,
        path: tmp.join("main.hnl").to_str().unwrap().to_string(),
    };

    let response = engine.completion(&request);
    let labels: Vec<_> = response.0.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| *l == "square"),
        "expected 'square' in {:?}",
        labels
    );
}
