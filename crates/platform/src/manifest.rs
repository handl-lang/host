//! Plugin manifest definitions and dependency declarations.

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
    /// Unique plugin identifier.
    pub id: String,
    /// Plugin version, defaulting to "0.0.0".
    #[serde(default = "default_version")]
    pub version: String,
    /// Capabilities this plugin provides.
    pub provides: Vec<CapabilityId>,
    /// Required capabilities or plugins.
    pub requires: Vec<Dependency>,
    /// Optional capabilities or plugins.
    pub optional: Vec<Dependency>,
    /// Plugins this plugin extends.
    pub extends: Vec<ExtendsDeclaration>,
    /// Declared conflicts with other plugins or capabilities.
    pub conflicts: Vec<ConflictDeclaration>,
    /// Plugins this plugin replaces.
    pub replaces: Vec<String>,
    /// Target runtime compatibility declarations.
    pub targets: Vec<TargetCompatibility>,
    /// Capabilities that this plugin declares may not be provided by multiple
    /// plugins in the final composition. The resolver rejects ambiguous
    /// exclusive providers unless a selector pins one.
    pub exclusive: Vec<CapabilityId>,
    /// Opaque loader-specific metadata, e.g. a native library `path`.
    #[serde(default)]
    pub metadata: JsonValue,
    /// AST kinds this plugin produces or consumes.
    #[serde(default)]
    pub ast_kinds: Vec<AstKindDeclaration>,
}

/// Whether a plugin produces or consumes an AST kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AstKindRole {
    /// The plugin produces AST nodes of this kind.
    Produces,
    /// The plugin consumes AST nodes of this kind.
    Consumes,
}

/// A declaration that a plugin produces or consumes a specific AST kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AstKindDeclaration {
    /// The AST node kind string (matches `AstNode.kind` or `Behavior::kind()`).
    pub kind: String,
    /// Whether this plugin produces or consumes the kind.
    pub role: AstKindRole,
}

/// A dependency on another capability or plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Dependency {
    /// A required capability.
    Capability(CapabilityDependency),
    /// A required plugin.
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

/// A dependency on a specific capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDependency {
    /// The required capability.
    pub capability: CapabilityId,
    /// Optional version requirement.
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

/// A dependency on a specific plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginDependency {
    /// The required plugin id.
    pub id: String,
    /// Optional version requirement.
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
    /// The plugin being extended.
    pub plugin: String,
    /// Optional version requirement of the extendee.
    pub version: Option<String>,
    /// Optional capability acting as the extension point.
    pub extension_point: Option<CapabilityId>,
}

/// Declaration that a plugin conflicts with another plugin or capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConflictDeclaration {
    /// The conflicting plugin, if any.
    pub plugin: Option<String>,
    /// The conflicting capability, if any.
    pub capability: Option<CapabilityId>,
    /// Human-readable reason for the conflict.
    pub reason: Option<String>,
}

/// Compatibility declaration for a particular target runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TargetCompatibility {
    /// The target runtime name.
    pub target: String,
    /// The support level for this target.
    pub level: TargetSupportLevel,
}

/// Level of support for a target runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetSupportLevel {
    /// Fully supported.
    Supported,
    /// Exactly matching semantics.
    Exact,
    /// Supported with no information loss.
    Lossless,
    /// Supported with possible information loss.
    Lossy,
    /// Verified at load time.
    Checked,
    /// Emulated with no guarantees.
    Emulated,
    /// Not supported.
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
            ast_kinds: Vec::new(),
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
        let roundtripped: PluginManifest = serde_json::from_str(&json).expect("deserialize");
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

    #[test]
    fn dependency_constructors_and_from_str() {
        let cap = Dependency::capability("a", Some("1.0".into()));
        assert!(matches!(cap, Dependency::Capability(_)));
        let plug = Dependency::plugin("b", Some(">=2.0".into()));
        assert!(matches!(plug, Dependency::Plugin(_)));

        let from: Dependency = "c".into();
        assert_eq!(from, Dependency::capability("c", None));

        let cap_dep: CapabilityDependency = "d".into();
        assert_eq!(cap_dep.capability, CapabilityId::from("d"));
        assert_eq!(cap_dep.version, None);

        let plug_dep: PluginDependency = "e".into();
        assert_eq!(plug_dep.id, "e");
        assert_eq!(plug_dep.version, None);
    }
}
