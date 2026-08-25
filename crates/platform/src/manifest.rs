use serde::{Deserialize, Serialize};

use crate::capability::CapabilityId;
use serde_json::Value as JsonValue;

fn default_version() -> String {
    "0.0.0".into()
}

/// A manifest describing a plugin and its relationships to other plugins and
/// capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PluginManifest {
    pub id: String,
    #[serde(default = "default_version")]
    pub version: String,
    pub provides: Vec<CapabilityId>,
    pub requires: Vec<Dependency>,
    pub optional: Vec<Dependency>,
    pub extends: Vec<ExtendsDeclaration>,
    pub conflicts: Vec<ConflictDeclaration>,
    pub replaces: Vec<String>,
    pub targets: Vec<TargetCompatibility>,
    /// Capabilities that this plugin declares may not be provided by multiple
    /// plugins in the final composition. The resolver rejects ambiguous
    /// exclusive providers unless a selector pins one.
    pub exclusive: Vec<CapabilityId>,
    /// Opaque loader-specific metadata, e.g. a native library `path`.
    #[serde(default)]
    pub metadata: JsonValue,
}

/// A dependency on another capability or plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Dependency {
    Capability(CapabilityDependency),
    Plugin(PluginDependency),
}

impl Dependency {
    /// Build a capability dependency.
    pub fn capability(id: impl Into<CapabilityId>, version: Option<String>) -> Self {
        Self::Capability(CapabilityDependency {
            capability: id.into(),
            version,
        })
    }

    /// Build a plugin dependency.
    pub fn plugin(id: impl Into<String>, version: Option<String>) -> Self {
        Self::Plugin(PluginDependency {
            id: id.into(),
            version,
        })
    }
}

impl From<&str> for Dependency {
    fn from(value: &str) -> Self {
        Self::capability(value, None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDependency {
    pub capability: CapabilityId,
    pub version: Option<String>,
}

impl From<&str> for CapabilityDependency {
    fn from(value: &str) -> Self {
        Self {
            capability: value.into(),
            version: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginDependency {
    pub id: String,
    pub version: Option<String>,
}

impl From<&str> for PluginDependency {
    fn from(value: &str) -> Self {
        Self {
            id: value.into(),
            version: None,
        }
    }
}

/// Declaration that a plugin extends another plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ExtendsDeclaration {
    pub plugin: String,
    pub version: Option<String>,
    pub extension_point: Option<CapabilityId>,
}

/// Declaration that a plugin conflicts with another plugin or capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConflictDeclaration {
    pub plugin: Option<String>,
    pub capability: Option<CapabilityId>,
    pub reason: Option<String>,
}

/// Compatibility declaration for a particular target runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TargetCompatibility {
    pub target: String,
    pub level: TargetSupportLevel,
}

/// Level of support for a target runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetSupportLevel {
    Supported,
    Exact,
    Lossless,
    Lossy,
    Checked,
    Emulated,
    Unsupported,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> PluginManifest {
        PluginManifest {
            id: "fmt".into(),
            version: "0.1.0".into(),
            provides: vec!["format".into()],
            requires: vec![
                Dependency::capability("compile", Some("^1.0".into())),
                Dependency::plugin("core", None),
            ],
            optional: vec!["lint".into()],
            extends: vec![ExtendsDeclaration {
                plugin: "editor".into(),
                version: Some(">=2.0".into()),
                extension_point: Some("format".into()),
            }],
            conflicts: vec![ConflictDeclaration {
                plugin: Some("legacy-fmt".into()),
                capability: None,
                reason: Some("replaces legacy formatter".into()),
            }],
            replaces: vec!["legacy-fmt".into()],
            targets: vec![
                TargetCompatibility {
                    target: "native".into(),
                    level: TargetSupportLevel::Supported,
                },
                TargetCompatibility {
                    target: "wasm".into(),
                    level: TargetSupportLevel::Checked,
                },
            ],
            exclusive: vec![],
            metadata: JsonValue::default(),
        }
    }

    #[test]
    fn manifest_construction() {
        let m = sample_manifest();
        assert_eq!(m.id, "fmt");
        assert_eq!(m.version, "0.1.0");
        assert_eq!(m.provides, vec![CapabilityId::from("format")]);
        assert_eq!(m.replaces, vec!["legacy-fmt"]);
        assert_eq!(m.targets[0].level, TargetSupportLevel::Supported);
    }

    #[test]
    fn json_round_trip() {
        let original = sample_manifest();
        let json = serde_json::to_string_pretty(&original).expect("serialize");
        let roundtripped: PluginManifest =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, roundtripped);
    }

    #[test]
    fn capability_dependency_parsing() {
        let dep: Dependency = "compile".into();
        assert_eq!(
            dep,
            Dependency::Capability(CapabilityDependency {
                capability: CapabilityId::from("compile"),
                version: None,
            })
        );

        let original = Dependency::capability("format", Some("1.2.3".into()));
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: Dependency = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, parsed);
    }

    #[test]
    fn plugin_dependency_parsing() {
        let original = Dependency::plugin("core", Some(">=2.0".into()));
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: Dependency = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, parsed);

        let from_str = PluginDependency::from("base");
        assert_eq!(from_str.id, "base");
        assert_eq!(from_str.version, None);
    }

    #[test]
    fn version_defaults_when_deserialized() {
        let json = r#"{"id":"x","provides":[],"requires":[],"optional":[],"extends":[],"conflicts":[],"replaces":[],"targets":[],"exclusive":[]}"#;
        let m: PluginManifest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(m.version, "0.0.0");
    }
}
