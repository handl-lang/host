use handl_platform::{CapabilityId, CapabilityRequest, PlatformError, Plugin, PluginContext};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;

use crate::ToolingHost;

/// Specification for a CLI argument.
#[derive(Debug, Clone, Default)]
pub struct ArgSpec {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub multiple: bool,
    pub allow_hyphen_values: bool,
}

impl ArgSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn multiple(mut self) -> Self {
        self.multiple = true;
        self
    }

    pub fn allow_hyphen_values(mut self) -> Self {
        self.allow_hyphen_values = true;
        self
    }
}

/// Specification for a CLI command.
#[derive(Debug, Clone, Default)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub args: Vec<ArgSpec>,
}

impl CommandSpec {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            ..Default::default()
        }
    }

    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    pub fn arg(mut self, arg: ArgSpec) -> Self {
        self.args.push(arg);
        self
    }
}

/// Reads a source string from the given file path, stdin (`-` or empty), or stdin.
pub fn read_source_or_stdin(file: Option<&str>) -> Result<String, PlatformError> {
    match file {
        Some(p) if p == "-" || p.is_empty() => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| PlatformError::Plugin(e.to_string()))?;
            Ok(buf)
        }
        Some(p) => std::fs::read_to_string(p)
            .map_err(|e| PlatformError::Plugin(e.to_string())),
        None => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| PlatformError::Plugin(e.to_string()))?;
            Ok(buf)
        }
    }
}

/// A provider of CLI commands.
pub trait CommandProvider: Send + Sync {
    /// Returns the command specs provided by this provider.
    fn commands(&self) -> Vec<CommandSpec>;

    /// Runs the given command with the supplied arguments.
    fn run(
        &self,
        command: &str,
        args: &[String],
        host: &ToolingHost,
    ) -> Result<Value, PlatformError>;

    /// Runs the given command with an owned tooling host.
    ///
    /// The default implementation delegates to [`CommandProvider::run`], borrowing the host.
    /// Providers that need to consume the host (e.g. the stdio LSP server) should override this.
    fn run_owned(
        &self,
        command: &str,
        args: Vec<String>,
        host: ToolingHost,
    ) -> Result<Value, PlatformError> {
        self.run(command, &args, &host)
    }
}

/// A command provider that dispatches to an externally loaded `Box<dyn Plugin>`
/// by mapping capabilities whose metadata declares a `command` field.
pub struct PluginCommandProvider {
    plugin: Box<dyn Plugin>,
    specs: Vec<CommandSpec>,
    by_command: HashMap<String, CapabilityId>,
}

impl PluginCommandProvider {
    pub fn new(plugin: Box<dyn Plugin>) -> Self {
        let mut specs = Vec::new();
        let mut by_command = HashMap::new();

        for cap in plugin.capabilities() {
            let Some(meta) = plugin.metadata(cap) else {
                continue;
            };
            let Some(command) = meta.get("command").and_then(|v| v.as_str()) else {
                continue;
            };
            let description = meta
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let aliases: Vec<String> = meta
                .get("aliases")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            let mut spec = CommandSpec::new(command, description);
            for alias in &aliases {
                spec = spec.alias(alias);
            }
            spec = spec.arg(
                ArgSpec::new("args")
                    .description("Command arguments")
                    .multiple()
                    .allow_hyphen_values(),
            );

            specs.push(spec);
            by_command.insert(command.to_string(), cap.clone());
            for alias in &aliases {
                by_command.insert(alias.clone(), cap.clone());
            }
        }

        Self {
            plugin,
            specs,
            by_command,
        }
    }
}

impl CommandProvider for PluginCommandProvider {
    fn commands(&self) -> Vec<CommandSpec> {
        self.specs.clone()
    }

    fn run(&self, command: &str, args: &[String], _host: &ToolingHost) -> Result<Value, PlatformError> {
        let capability = self
            .by_command
            .get(command)
            .ok_or_else(|| PlatformError::UnknownCapability(command.into()))?;

        let mut plugins = handl_platform::PluginRegistry::new();
        let mut kits = handl_platform::KitRegistry::new();
        let mut extensions = handl_platform::ExtensionRegistry::new();
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
        };

        let request = CapabilityRequest {
            capability: capability.clone(),
            payload: serde_json::json!({ "args": args }),
        };

        let response = self.plugin.run(&request, &mut ctx)?;
        Ok(response.0)
    }
}
