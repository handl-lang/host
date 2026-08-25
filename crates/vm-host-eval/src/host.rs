use handl_platform::DefaultHost;
use handl_vm::{Function, Host};
use handl_vm_values_json::NonFunctionValue;

use crate::functions::{Apply, Resolve, Var};

/// The eval host implementation for the handl VM.
///
/// Provides built-in functions (`var`, `resolve`, and `apply`) that are
/// available in all handl expressions. It also acts as a `handl_platform`
/// capability host by containing a `DefaultHost` with the platform's
/// plugin, kit, and extension registries.
pub struct EvalHost {
    pub var: Var,
    pub resolve: Resolve,
    pub apply: Apply,
    pub platform: DefaultHost,
}

impl Default for EvalHost {
    fn default() -> Self {
        EvalHost {
            var: Var,
            resolve: Resolve,
            apply: Apply,
            platform: DefaultHost::default(),
        }
    }
}

impl Clone for EvalHost {
    fn clone(&self) -> Self {
        Self {
            var: self.var.clone(),
            resolve: self.resolve.clone(),
            apply: self.apply.clone(),
            platform: DefaultHost::default(),
        }
    }
}

impl handl_platform::host::Host for EvalHost {
    fn plugins(&self) -> &handl_platform::PluginRegistry {
        self.platform.plugins()
    }

    fn kits(&self) -> &handl_platform::KitRegistry {
        self.platform.kits()
    }

    fn extensions(&self) -> &handl_platform::ExtensionRegistry {
        self.platform.extensions()
    }
}

impl Host<NonFunctionValue> for EvalHost {
    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "var" => Some(&self.var),
            "resolve" => Some(&self.resolve),
            "apply" => Some(&self.apply),
            _ => None,
        }
    }

    fn is_reserved(&self, name: &str) -> bool {
        self.function(name).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::host::Host as PlatformHost;
    use handl_platform::{
        CapabilityId, CapabilityRequest, CapabilityResponse, PlatformError, Plugin, PluginContext,
    };

    #[test]
    fn default_initializes_vm_functions_and_empty_platform() {
        let host = EvalHost::default();
        assert!(host.function("var").is_some());
        assert!(host.function("resolve").is_some());
        assert!(host.function("apply").is_some());
        assert!(host.function("foo").is_none());
        assert!(host.platform.plugins().capabilities().is_empty());
        assert!(host.platform.kits().all().is_empty());
        assert!(host.platform.extensions().all().is_empty());
    }

    #[test]
    fn platform_host_methods_return_registries() {
        let host = EvalHost::default();
        let _: &dyn PlatformHost = &host;
        assert!(host.plugins().capabilities().is_empty());
        assert!(host.kits().all().is_empty());
        assert!(host.extensions().all().is_empty());
    }

    struct EchoPlugin {
        capabilities: Vec<CapabilityId>,
    }

    impl EchoPlugin {
        fn new() -> Self {
            Self {
                capabilities: vec![CapabilityId::from("echo")],
            }
        }
    }

    impl Plugin for EchoPlugin {
        fn name(&self) -> &str {
            "echo"
        }

        fn capabilities(&self) -> &[CapabilityId] {
            &self.capabilities
        }

        fn metadata(&self, capability: &CapabilityId) -> Option<handl_platform::CapabilityMetadata> {
            self.capabilities
                .contains(capability)
                .then(|| serde_json::json!({ "command": "echo" }))
        }

        fn run(
            &self,
            request: &CapabilityRequest,
            _ctx: &mut PluginContext,
        ) -> Result<CapabilityResponse, PlatformError> {
            Ok(CapabilityResponse(serde_json::json!({
                "echoed": request.capability.as_str(),
                "payload": request.payload,
            })))
        }
    }

    #[test]
    fn platform_plugin_runs_through_canonical_host() {
        let mut host = EvalHost::default();
        host.platform.register_plugin(EchoPlugin::new());
        let request = CapabilityRequest {
            capability: CapabilityId::from("echo"),
            payload: serde_json::json!({ "hello": "world" }),
        };
        let response = host.platform.run(&request).unwrap();
        assert_eq!(
            response.0,
            serde_json::json!({
                "echoed": "echo",
                "payload": { "hello": "world" },
            })
        );
    }
}
