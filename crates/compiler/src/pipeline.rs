//! JSON-oriented, platform-integrated compiler pipeline.
//!
//! `PipelineBuilder` collects [`Pass`] objects from a `handl-platform`
//! `DefaultHost` for each compiler stage and assembles a `Pipeline` that
//! runs them in order.

use std::sync::Arc;

use serde_json::Value;

use crate::{CompilerError, Pass, PassHost};
use handl_platform::{
    CapabilityId, CapabilityRequest, CapabilityResponse, DefaultHost, ExtensionRegistry, Host,
    KitRegistry, Plugin, PluginContext, PluginRegistry,
};

/// A configured pipeline that runs five stages in order.
pub struct Pipeline {
    parse: PassHost,
    extract: PassHost,
    resolve: PassHost,
    typecheck: PassHost,
    generate: PassHost,
}

impl Pipeline {
    /// Run the five stage pass hosts in order.
    pub fn run(&self, input: Value) -> Result<Value, CompilerError> {
        let x = self.parse.run(input)?;
        let x = self.extract.run(x)?;
        let x = self.resolve.run(x)?;
        let x = self.typecheck.run(x)?;
        self.generate.run(x)
    }
}

/// Builder that collects `Pass` objects from a `DefaultHost` and assembles
/// a `Pipeline`.
pub struct PipelineBuilder<'h> {
    host: &'h DefaultHost,
}

impl<'h> PipelineBuilder<'h> {
    /// Create a builder tied to the given host.
    pub fn new(host: &'h DefaultHost) -> Self {
        Self { host }
    }

    fn stage_host(&self, stage: &'static str) -> PassHost {
        let mut host = PassHost::new();
        let capability = CapabilityId::from(stage);
        if let Some(plugin) = self.host.plugins().resolve(&capability) {
            host.register(PlatformPass::new(plugin, capability));
        }
        host
    }

    /// Build the `Pipeline`.
    pub fn build(self) -> Pipeline {
        Pipeline {
            parse: self.stage_host("parse"),
            extract: self.stage_host("extract"),
            resolve: self.stage_host("resolve"),
            typecheck: self.stage_host("typecheck"),
            generate: self.stage_host("generate"),
        }
    }
}

/// A `Pass` adapter that invokes a platform `Plugin` for a single stage.
struct PlatformPass {
    plugin: Arc<dyn Plugin>,
    capability: CapabilityId,
}

impl PlatformPass {
    fn new(plugin: Arc<dyn Plugin>, capability: CapabilityId) -> Self {
        Self { plugin, capability }
    }
}

impl Pass for PlatformPass {
    fn id(&self) -> &str {
        self.capability.as_str()
    }

    fn run(&self, input: Value) -> Result<Value, CompilerError> {
        let request = CapabilityRequest {
            capability: self.capability.clone(),
            payload: input,
        };
        let mut plugins = PluginRegistry::new();
        let mut kits = KitRegistry::new();
        let mut extensions = ExtensionRegistry::new();
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
        };
        match self.plugin.run(&request, &mut ctx) {
            Ok(CapabilityResponse(output)) => Ok(output),
            Err(e) => Err(CompilerError::PassFailed(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{InProcessPlugin, PluginContext};

    fn stage_plugin(stage: &'static str) -> InProcessPlugin<impl Fn(&CapabilityRequest, &mut PluginContext) -> Result<CapabilityResponse, handl_platform::PlatformError>> {
        InProcessPlugin::new(
            stage,
            &[stage.into()],
            serde_json::json!({"command": stage, "description": "mock"}),
            move |req, _ctx| {
                let mut obj = match req.payload.clone() {
                    Value::Object(m) => m,
                    _ => serde_json::Map::new(),
                };
                obj.insert(stage.to_string(), Value::Bool(true));
                Ok(CapabilityResponse(Value::Object(obj)))
            },
        )
    }

    #[test]
    fn pipeline_runs_registered_stages_in_order() {
        let mut host = DefaultHost::new();
        host.register_plugin(stage_plugin("parse"));
        host.register_plugin(stage_plugin("extract"));
        host.register_plugin(stage_plugin("resolve"));
        host.register_plugin(stage_plugin("typecheck"));
        host.register_plugin(stage_plugin("generate"));

        let pipeline = PipelineBuilder::new(&host).build();
        let out = pipeline.run(serde_json::json!({})).unwrap();

        assert!(out.get("parse").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(out.get("extract").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(out.get("resolve").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(out.get("typecheck").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(out.get("generate").and_then(|v| v.as_bool()).unwrap_or(false));
    }

    #[test]
    fn missing_stage_is_pass_through() {
        let mut host = DefaultHost::new();
        host.register_plugin(stage_plugin("parse"));
        host.register_plugin(stage_plugin("generate"));

        let pipeline = PipelineBuilder::new(&host).build();
        let out = pipeline.run(serde_json::json!({"start": true})).unwrap();

        assert!(out.get("start").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(out.get("parse").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(out.get("generate").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(out.get("extract").is_none());
    }

    #[test]
    fn pipeline_error_propagates() {
        let mut host = DefaultHost::new();
        host.register_plugin(stage_plugin("parse"));
        host.register_plugin(InProcessPlugin::new(
            "extract",
            &["extract".into()],
            serde_json::json!({"command": "extract"}),
            |_req, _ctx| Err(handl_platform::PlatformError::Plugin("fail".to_string())),
        ));

        let pipeline = PipelineBuilder::new(&host).build();
        let err = pipeline.run(serde_json::json!({})).unwrap_err();
        assert!(matches!(err, CompilerError::PassFailed(_)));
    }
}
