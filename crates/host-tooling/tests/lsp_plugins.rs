use handl_host_tooling::{Location, TextContentChange, ToolingHost};
use handl_plugin_lsp_completion::BasicCompletion;
use handl_plugin_lsp_goto::BasicGotoDef;
use handl_plugin_lsp_symbol::BasicDocumentSymbols;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};

/// Minimal stub kit plugin (replaces the removed `CanonicalKit`).
struct StubKit;

impl Plugin for StubKit {
    fn id(&self) -> &str {
        "stub.kit"
    }
    fn name(&self) -> &str {
        "stub-kit"
    }
    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }
    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }
    fn contribute(&self, _ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        Ok(())
    }
    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

fn host() -> ToolingHost<StubKit> {
    ToolingHost::new(StubKit)
        .with_completion_plugin(Box::new(BasicCompletion::new()))
        .with_goto_def_plugin(Box::new(BasicGotoDef::new()))
        .with_document_symbols_plugin(Box::new(BasicDocumentSymbols::new()))
}

#[test]
#[ignore = "LSP plugins are stubbed"]
fn complete_includes_let_binding() {
    let h = host();
    let items = h.complete("a = 1; a", 8).unwrap();
    assert!(
        items.iter().any(|i| i.label == "a"),
        "expected binding 'a' in {items:?}"
    );
}

#[test]
#[ignore = "LSP plugins are stubbed"]
fn complete_offers_keyword() {
    let h = host();
    let items = h.complete("", 0).unwrap();
    assert!(items.iter().any(|i| i.label == "match"));
}

#[test]
#[ignore = "LSP plugins are stubbed"]
fn goto_def_finds_let_binding() {
    let h = host();
    let src = "a = 1; a";
    let loc = h
        .goto_def(src, 7)
        .unwrap()
        .expect("goto_def should return a location");
    assert_eq!(loc, Location::new(4, 5));
}

#[test]
#[ignore = "LSP plugins are stubbed"]
fn document_symbols_for_let_and_const() {
    let h = host();
    let src = "a = 1; b = 2;";
    let syms = h.document_symbols(src).unwrap();
    assert_eq!(syms.len(), 2);
    assert_eq!(syms[0].name, "a");
    assert_eq!(syms[1].name, "b");
}

#[test]
#[ignore = "LSP plugins are stubbed"]
fn incremental_change_appends_text() {
    let mut h = host();
    let src = "a = 1; a";
    let new = h
        .change(src, TextContentChange::insert(8, " + 2"))
        .unwrap();
    assert_eq!(new, "a = 1; a + 2");
}

#[test]
#[ignore = "LSP plugins are stubbed"]
fn format_still_works() {
    let h = host();
    let out = h.format("a = 1").unwrap();
    assert_eq!(out, "a = 1;");
}
