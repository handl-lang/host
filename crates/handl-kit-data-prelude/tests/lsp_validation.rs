use handl_kit_data_prelude::language_from_yaml;
use handl_plugin_ext_diag::Severity;

#[test]
fn canonical_lsp_validates_clean() {
    let lang = language_from_yaml().expect("canonical language assembles");
    let diags = hnl_engine_transform::validate::validate_lsp(&lang);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "unexpected lsp validation errors: {errors:?}"
    );
}
