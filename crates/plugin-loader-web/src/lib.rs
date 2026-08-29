use handl_plugin_load::{LoaderHostPlugin, LoaderRule, PluginLoader};
use handl_platform::{
    CapabilityId, CapabilityRequest, CapabilityResponse, ContributionContext, PlatformError, Plugin,
    PluginContext, PluginManifest,
};

#[derive(Clone, Default)]
pub struct WebLoader;

impl WebLoader {
    pub fn new() -> Self {
        Self
    }

    fn resolve_url(&self, manifest: &PluginManifest) -> Option<String> {
        if let Some(url) = manifest
            .metadata
            .get("url")
            .and_then(|value| value.as_str())
        {
            return Some(url.to_string());
        }

        if manifest.id.starts_with("http://") || manifest.id.starts_with("https://") {
            Some(manifest.id.clone())
        } else {
            None
        }
    }
}

impl PluginLoader for WebLoader {
    fn can_load(&self, manifest: &PluginManifest) -> bool {
        self.resolve_url(manifest).is_some()
    }

    fn load(&self, manifest: &PluginManifest) -> Result<Box<dyn Plugin + 'static>, PlatformError> {
        let url = self
            .resolve_url(manifest)
            .ok_or_else(|| PlatformError::Plugin("manifest has no web url".to_string()))?;

        Ok(Box::new(SharedWebPlugin {
            manifest: manifest.clone(),
            url,
        }))
    }
}

impl Plugin for WebLoader {
    fn name(&self) -> &str {
        "loader-web"
    }

    fn id(&self) -> &str {
        "loader.web"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<serde_json::Value> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if ctx.get_plugin_mut::<LoaderHostPlugin>().is_none() {
            ctx.register_plugin(LoaderHostPlugin::default())?;
        }
        ctx.contribute::<LoaderHostPlugin>(LoaderRule {
            id: self.name().to_string(),
            loader: Box::new(self.clone()),
        })
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::UnknownCapability(request.capability.clone()))
    }
}

struct SharedWebPlugin {
    manifest: PluginManifest,
    url: String,
}

impl Plugin for SharedWebPlugin {
    fn name(&self) -> &str {
        &self.manifest.id
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.manifest.provides
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<serde_json::Value> {
        None
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        let request_json = serde_json::to_string(request)
            .map_err(|e| PlatformError::Plugin(format!("failed to serialize request: {e}")))?;

        let response = ureq::post(&self.url)
            .set("Content-Type", "application/json")
            .send_string(&request_json)
            .map_err(|e| PlatformError::Plugin(format!("http post failed: {e}")))?;

        let response_text = response
            .into_string()
            .map_err(|e| PlatformError::Plugin(format!("failed to read http response: {e}")))?;
        let value = serde_json::from_str(&response_text)
            .map_err(|e| PlatformError::Plugin(format!("invalid response json: {e}")))?;

        Ok(CapabilityResponse(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_loader_post_and_echo() {
        let mut server = mockito::Server::new();
        let url = server.url();

        let _m = server
            .mock("POST", "/")
            .match_header("content-type", "application/json")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "capability": "echo",
                "payload": {"hello": "world"},
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&serde_json::json!({
                "echoed": "echo",
                "payload": {"hello": "world"},
            })).unwrap())
            .create();

        let manifest = PluginManifest {
            id: url.clone(),
            version: "0.1.0".to_string(),
            provides: vec!["echo".into()],
            requires: vec![],
            optional: vec![],
            extends: vec![],
            conflicts: vec![],
            replaces: vec![],
            targets: vec![],
            exclusive: vec![],
            metadata: serde_json::Value::default(),
        };

        let loader = WebLoader::new();
        assert!(loader.can_load(&manifest));
        let plugin = loader.load(&manifest).expect("load web");
        assert!(plugin.capabilities().contains(&CapabilityId::from("echo")));

        let (mut plugins, mut kits) = (
            handl_platform::PluginRegistry::new(),
            handl_platform::KitRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };

        let response = plugin
            .run(
                &CapabilityRequest {
                    capability: "echo".into(),
                    payload: serde_json::json!({"hello": "world"}),
                },
                &mut ctx,
            )
            .expect("web echo");

        assert_eq!(response.0["payload"]["hello"], "world");
    }
}
