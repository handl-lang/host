//! Plugin composition resolution.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::capability::CapabilityId;
use crate::manifest::{
    CapabilityDependency, Dependency, PluginDependency, PluginManifest, TargetSupportLevel,
};

/// Selects a specific plugin to satisfy a capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CapabilitySelector {
    /// The capability to pin.
    pub capability: CapabilityId,
    /// The chosen provider plugin id.
    pub provider: String,
}

/// The result of resolving a plugin composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedComposition {
    /// The ids of selected plugins.
    pub selected_plugins: Vec<String>,
    /// Map from each capability to its selected provider.
    pub provider_map: HashMap<CapabilityId, String>,
    /// Extender-to-extendee wirings.
    pub extension_wirings: Vec<ExtensionWiring>,
    /// Topological initialization order.
    pub initialization_order: Vec<String>,
    /// Optional dependencies that could not be resolved.
    pub missing_optional: Vec<Dependency>,
    /// Target support level for each selected plugin.
    pub target_report: HashMap<String, TargetSupportLevel>,
}

/// A wiring from an extender plugin to the plugin it extends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionWiring {
    /// The plugin that extends.
    pub extender: String,
    /// The plugin being extended.
    pub extendee: String,
    /// The capability extension point, if specified.
    pub extension_point: Option<CapabilityId>,
}

/// Errors that can occur during composition resolution.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum ResolutionError {
    /// A required capability is missing.
    #[error("missing required capability {capability} (required by {required_by})")]
    MissingCapability {
        /// The missing capability.
        capability: CapabilityId,
        /// The plugin that required it.
        required_by: String,
    },
    /// A required plugin is missing.
    #[error("missing required plugin {plugin} (required by {required_by})")]
    MissingPlugin {
        /// The missing plugin.
        plugin: String,
        /// The plugin that required it.
        required_by: String,
    },
    /// An exclusive capability has multiple providers.
    #[error("multiple providers for exclusive capability {capability}: {providers:?}")]
    MultipleProvidersForExclusiveCapability {
        /// The exclusive capability.
        capability: CapabilityId,
        /// The competing providers.
        providers: Vec<String>,
    },
    /// A version requirement is not satisfied.
    #[error(
        "version mismatch for {provider} providing {capability}: requires {required}, found {found}"
    )]
    VersionMismatch {
        /// The capability with the mismatch.
        capability: CapabilityId,
        /// The selected provider.
        provider: String,
        /// The required version.
        required: String,
        /// The found version.
        found: String,
    },
    /// A conflict exists between selected plugins.
    #[error("conflict between selected plugins {plugin_a} and {plugin_b}")]
    Conflict {
        /// The first conflicting plugin.
        plugin_a: String,
        /// The second conflicting plugin.
        plugin_b: String,
    },
    /// A cycle was found in the dependency or extends graph.
    #[error("cyclic extends/dependency graph: {cycle:?}")]
    CyclicGraph {
        /// The plugins in the cycle.
        cycle: Vec<String>,
    },
    /// A selector does not provide the selected capability.
    #[error("selector {provider} does not provide capability {capability}")]
    InvalidSelector {
        /// The selected capability.
        capability: CapabilityId,
        /// The chosen provider.
        provider: String,
    },
}

/// Resolves a composition of plugins from the available manifests.
///
/// All manifests are considered candidates for selection. Required dependencies
/// must be satisfied; missing optional dependencies are recorded in
/// `missing_optional`. Selectors pin the provider for a capability, overriding
/// the default choice.
pub fn resolve(
    manifests: &[PluginManifest],
    selectors: &[CapabilitySelector],
    target: Option<&str>,
) -> Result<ResolvedComposition, ResolutionError> {
    let by_id: HashMap<String, &PluginManifest> =
        manifests.iter().map(|m| (m.id.clone(), m)).collect();

    // Detect multiple selected plugins with the same id but different versions.
    let mut seen_versions: HashMap<String, &str> = HashMap::new();
    for m in manifests {
        if let Some(v) = seen_versions.get(&m.id) {
            if v != &m.version.as_str() {
                return Err(ResolutionError::Conflict {
                    plugin_a: format!("{}@{}", m.id, m.version),
                    plugin_b: format!("{}@{}", m.id, v),
                });
            }
        } else {
            seen_versions.insert(m.id.clone(), &m.version);
        }
    }

    // Index capability providers and exclusive capabilities.
    let mut providers: HashMap<CapabilityId, Vec<&PluginManifest>> = HashMap::new();
    let mut exclusive: HashSet<CapabilityId> = HashSet::new();
    for m in manifests {
        for cap in &m.provides {
            providers.entry(cap.clone()).or_default().push(m);
        }
        for cap in &m.exclusive {
            exclusive.insert(cap.clone());
        }
    }

    let selected: HashSet<String> = manifests.iter().map(|m| m.id.clone()).collect();
    let selector_by_cap: HashMap<CapabilityId, &str> = selectors
        .iter()
        .map(|s| (s.capability.clone(), s.provider.as_str()))
        .collect();

    // Build the provider map, validating selectors and exclusive constraints.
    let mut provider_map: HashMap<CapabilityId, String> = HashMap::new();
    for (cap, cap_providers) in &providers {
        if let Some(&sel_provider) = selector_by_cap.get(cap) {
            let m = by_id
                .get(sel_provider)
                .ok_or_else(|| ResolutionError::MissingPlugin {
                    plugin: sel_provider.to_string(),
                    required_by: format!("selector for {cap}"),
                })?;
            if !m.provides.contains(cap) {
                return Err(ResolutionError::InvalidSelector {
                    capability: cap.clone(),
                    provider: sel_provider.to_string(),
                });
            }
            provider_map.insert(cap.clone(), sel_provider.to_string());
        } else if cap_providers.len() == 1 {
            provider_map.insert(cap.clone(), cap_providers[0].id.clone());
        } else if cap_providers.len() > 1 {
            if exclusive.contains(cap) {
                return Err(ResolutionError::MultipleProvidersForExclusiveCapability {
                    capability: cap.clone(),
                    providers: cap_providers.iter().map(|m| m.id.clone()).collect(),
                });
            }
            // Pick the provider with the highest semantic version.
            let mut best = cap_providers[0];
            for m in cap_providers.iter().skip(1) {
                if version_cmp(&m.version, &best.version) == std::cmp::Ordering::Greater {
                    best = m;
                }
            }
            provider_map.insert(cap.clone(), best.id.clone());
        }
    }

    // Resolve required dependencies (capabilities and plugins).
    for m in manifests {
        for dep in &m.requires {
            match dep {
                Dependency::Capability(cap) => {
                    resolve_capability_dependency(&m.id, cap, &provider_map, &by_id)?;
                }
                Dependency::Plugin(p) => {
                    resolve_plugin_dependency(&m.id, p, &by_id)?;
                }
            }
        }
    }

    // Resolve optional dependencies; record missing/unresolved ones.
    let mut missing_optional = Vec::new();
    for m in manifests {
        for dep in &m.optional {
            let resolved = match dep {
                Dependency::Capability(cap) => {
                    optional_capability_resolved(cap, &provider_map, &by_id)
                }
                Dependency::Plugin(p) => optional_plugin_resolved(p, &by_id),
            };
            if !resolved {
                missing_optional.push(dep.clone());
            }
        }
    }

    // Check explicit conflict declarations.
    check_conflicts(manifests, &selected, &provider_map)?;

    // Build extension wirings.
    let mut extension_wirings = Vec::new();
    for m in manifests {
        for ext in &m.extends {
            if !by_id.contains_key(&ext.plugin) {
                return Err(ResolutionError::MissingPlugin {
                    plugin: ext.plugin.clone(),
                    required_by: m.id.clone(),
                });
            }
            extension_wirings.push(ExtensionWiring {
                extender: m.id.clone(),
                extendee: ext.plugin.clone(),
                extension_point: ext.extension_point.clone(),
            });
        }
    }

    // Topological initialization order over requires and extends edges.
    let init_order = topological_init_order(manifests, &selected, &provider_map)?;

    // Target support report for the requested target.
    let mut target_report = HashMap::new();
    if let Some(t) = target {
        for m in manifests {
            let level = m
                .targets
                .iter()
                .find(|tc| tc.target == t)
                .map(|tc| tc.level.clone())
                .unwrap_or(TargetSupportLevel::Unsupported);
            target_report.insert(m.id.clone(), level);
        }
    }

    let mut selected_plugins: Vec<String> = selected.iter().cloned().collect();
    selected_plugins.sort();

    Ok(ResolvedComposition {
        selected_plugins,
        provider_map,
        extension_wirings,
        initialization_order: init_order,
        missing_optional,
        target_report,
    })
}

fn resolve_capability_dependency(
    required_by: &str,
    dep: &CapabilityDependency,
    provider_map: &HashMap<CapabilityId, String>,
    by_id: &HashMap<String, &PluginManifest>,
) -> Result<(), ResolutionError> {
    let provider_id =
        provider_map
            .get(&dep.capability)
            .ok_or_else(|| ResolutionError::MissingCapability {
                capability: dep.capability.clone(),
                required_by: required_by.to_string(),
            })?;
    let provider = by_id
        .get(provider_id)
        .ok_or_else(|| ResolutionError::MissingPlugin {
            plugin: provider_id.clone(),
            required_by: required_by.to_string(),
        })?;
    if let Some(req) = &dep.version
        && !version_satisfies(req, &provider.version)
    {
        return Err(ResolutionError::VersionMismatch {
            capability: dep.capability.clone(),
            provider: provider_id.clone(),
            required: req.clone(),
            found: provider.version.clone(),
        });
    }
    Ok(())
}

fn resolve_plugin_dependency(
    required_by: &str,
    dep: &PluginDependency,
    by_id: &HashMap<String, &PluginManifest>,
) -> Result<(), ResolutionError> {
    let target = by_id
        .get(&dep.id)
        .ok_or_else(|| ResolutionError::MissingPlugin {
            plugin: dep.id.clone(),
            required_by: required_by.to_string(),
        })?;
    if let Some(req) = &dep.version
        && !version_satisfies(req, &target.version)
    {
        return Err(ResolutionError::VersionMismatch {
            capability: dep.id.clone().into(),
            provider: dep.id.clone(),
            required: req.clone(),
            found: target.version.clone(),
        });
    }
    Ok(())
}

fn optional_capability_resolved(
    dep: &CapabilityDependency,
    provider_map: &HashMap<CapabilityId, String>,
    by_id: &HashMap<String, &PluginManifest>,
) -> bool {
    let Some(provider_id) = provider_map.get(&dep.capability) else {
        return false;
    };
    let Some(provider) = by_id.get(provider_id) else {
        return false;
    };
    if let Some(req) = &dep.version
        && !version_satisfies(req, &provider.version)
    {
        return false;
    }
    true
}

fn optional_plugin_resolved(
    dep: &PluginDependency,
    by_id: &HashMap<String, &PluginManifest>,
) -> bool {
    let Some(target) = by_id.get(&dep.id) else {
        return false;
    };
    if let Some(req) = &dep.version
        && !version_satisfies(req, &target.version)
    {
        return false;
    }
    true
}

fn check_conflicts(
    manifests: &[PluginManifest],
    selected: &HashSet<String>,
    provider_map: &HashMap<CapabilityId, String>,
) -> Result<(), ResolutionError> {
    for m in manifests {
        if !selected.contains(&m.id) {
            continue;
        }
        for conflict in &m.conflicts {
            if let Some(plugin_id) = &conflict.plugin
                && plugin_id != &m.id
                && selected.contains(plugin_id)
            {
                return Err(ResolutionError::Conflict {
                    plugin_a: m.id.clone(),
                    plugin_b: plugin_id.clone(),
                });
            }
            if let Some(cap) = &conflict.capability
                && let Some(provider_id) = provider_map.get(cap)
                && provider_id != &m.id
                && selected.contains(provider_id)
            {
                return Err(ResolutionError::Conflict {
                    plugin_a: m.id.clone(),
                    plugin_b: provider_id.clone(),
                });
            }
        }
    }
    Ok(())
}

fn topological_init_order(
    manifests: &[PluginManifest],
    selected: &HashSet<String>,
    provider_map: &HashMap<CapabilityId, String>,
) -> Result<Vec<String>, ResolutionError> {
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new(); // B -> [A] means A depends on B

    for id in selected {
        in_degree.entry(id.clone()).or_insert(0);
        dependents.entry(id.clone()).or_default();
    }

    let mut add_edge = |from: &str, to: &str| {
        if from == to || !selected.contains(to) {
            return;
        }
        if let Some(d) = in_degree.get_mut(from) {
            *d += 1;
        }
        dependents
            .entry(to.to_string())
            .or_default()
            .push(from.to_string());
    };

    for m in manifests {
        if !selected.contains(&m.id) {
            continue;
        }
        // Required plugin and capability dependencies become edges.
        for dep in &m.requires {
            let target = match dep {
                Dependency::Capability(cap) => provider_map.get(&cap.capability).cloned(),
                Dependency::Plugin(p) => Some(p.id.clone()),
            };
            if let Some(t) = target {
                add_edge(&m.id, &t);
            }
        }
        // Extends declarations become edges from extender to extendee.
        for ext in &m.extends {
            if selected.contains(&ext.plugin) {
                add_edge(&m.id, &ext.plugin);
            }
        }
    }

    // Kahn's algorithm using a BinaryHeap for deterministic order.
    let mut heap: BinaryHeap<Reverse<String>> = in_degree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(id, _)| Reverse(id.clone()))
        .collect();
    let mut order = Vec::new();

    while let Some(Reverse(id)) = heap.pop() {
        order.push(id.clone());
        for dep in dependents.get(&id).cloned().unwrap_or_default() {
            if let Some(d) = in_degree.get_mut(&dep) {
                *d -= 1;
                if *d == 0 {
                    heap.push(Reverse(dep));
                }
            }
        }
    }

    if order.len() != selected.len() {
        // Build a simple adjacency for the dependency graph and find a cycle.
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        for m in manifests {
            if !selected.contains(&m.id) {
                continue;
            }
            let mut targets = Vec::new();
            for dep in &m.requires {
                match dep {
                    Dependency::Capability(cap) => {
                        if let Some(p) = provider_map.get(&cap.capability) {
                            targets.push(p.clone());
                        }
                    }
                    Dependency::Plugin(p) => targets.push(p.id.clone()),
                }
            }
            for ext in &m.extends {
                targets.push(ext.plugin.clone());
            }
            adj.insert(m.id.clone(), targets);
        }
        let cycle = find_cycle(&adj).unwrap_or_default();
        return Err(ResolutionError::CyclicGraph { cycle });
    }

    Ok(order)
}

fn find_cycle(adj: &HashMap<String, Vec<String>>) -> Option<Vec<String>> {
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    let mut on_stack = HashSet::new();

    for start in adj.keys() {
        if !visited.contains(start)
            && let Some(cycle) = dfs_cycle(start, adj, &mut visited, &mut stack, &mut on_stack)
        {
            return Some(cycle);
        }
    }
    None
}

fn dfs_cycle(
    node: &str,
    adj: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    stack: &mut Vec<String>,
    on_stack: &mut HashSet<String>,
) -> Option<Vec<String>> {
    visited.insert(node.to_string());
    stack.push(node.to_string());
    on_stack.insert(node.to_string());

    for next in adj.get(node).cloned().unwrap_or_default() {
        if on_stack.contains(&next)
            && let Some(pos) = stack.iter().position(|n| n == &next)
        {
            return Some(stack[pos..].to_vec());
        }
        if !visited.contains(&next)
            && let Some(cycle) = dfs_cycle(&next, adj, visited, stack, on_stack)
        {
            return Some(cycle);
        }
    }

    stack.pop();
    on_stack.remove(node);
    None
}

fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    match (Version::parse(a), Version::parse(b)) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        _ => version_tuple(a).cmp(&version_tuple(b)),
    }
}

fn normalize_version_req(req: &str) -> Option<String> {
    let req = req.trim();
    let (op, ver) = if let Some(prefix) = [">=", "<=", ">", "<", "^", "~", "="]
        .iter()
        .find(|&&p| req.starts_with(p))
    {
        (&prefix[..], req[prefix.len()..].trim())
    } else {
        ("=", req)
    };
    if ver.is_empty() {
        return None;
    }
    let parts: Vec<&str> = ver.split('.').collect();
    let padded = match parts.len() {
        1 => format!("{}.0.0", parts[0]),
        2 => format!("{}.{}", ver, "0"),
        _ => ver.to_string(),
    };
    Some(format!("{}{}", op, padded))
}

fn version_satisfies(required: &str, found: &str) -> bool {
    let required = required.trim();
    let found = found.trim();
    if required == found {
        return true;
    }
    if let Ok(found_v) = Version::parse(found)
        && let Some(normalized) = normalize_version_req(required)
        && let Ok(req) = VersionReq::parse(&normalized)
    {
        return req.matches(&found_v);
    }
    // Fallback to exact string match.
    required == found
}

fn version_tuple(v: &str) -> Vec<u64> {
    v.split('.').filter_map(|s| s.parse::<u64>().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ExtendsDeclaration;
    use crate::{ConflictDeclaration, TargetCompatibility, TargetSupportLevel};

    fn manifest(id: &str, version: &str, provides: &[&str]) -> PluginManifest {
        PluginManifest {
            id: id.into(),
            version: version.into(),
            provides: provides.iter().map(|&c| c.into()).collect(),
            requires: Vec::new(),
            optional: Vec::new(),
            extends: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
            targets: Vec::new(),
            exclusive: Vec::new(),
            metadata: serde_json::Value::default(),
        }
    }

    #[test]
    fn simple_capability_satisfaction() {
        let manifests = vec![
            PluginManifest {
                id: "fmt".into(),
                version: "0.1.0".into(),
                provides: vec!["format".into()],
                requires: vec![Dependency::capability("compile", None)],
                optional: Vec::new(),
                extends: Vec::new(),
                conflicts: Vec::new(),
                replaces: Vec::new(),
                targets: Vec::new(),
                exclusive: Vec::new(),
                metadata: serde_json::Value::default(),
            },
            PluginManifest {
                id: "compile".into(),
                version: "1.0.0".into(),
                provides: vec!["compile".into()],
                requires: Vec::new(),
                optional: Vec::new(),
                extends: Vec::new(),
                conflicts: Vec::new(),
                replaces: Vec::new(),
                targets: Vec::new(),
                exclusive: Vec::new(),
                metadata: serde_json::Value::default(),
            },
        ];
        let resolved = resolve(&manifests, &[], None).unwrap();
        assert_eq!(resolved.selected_plugins, vec!["compile", "fmt"]);
        assert_eq!(
            resolved.provider_map.get(&"compile".into()),
            Some(&"compile".to_string())
        );
        assert!(resolved.missing_optional.is_empty());
    }

    #[test]
    fn missing_capability_error() {
        let manifests = vec![PluginManifest {
            id: "fmt".into(),
            version: "0.1.0".into(),
            provides: vec!["format".into()],
            requires: vec![Dependency::capability("missing-cap", None)],
            optional: Vec::new(),
            extends: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
            targets: Vec::new(),
            exclusive: Vec::new(),
            metadata: serde_json::Value::default(),
        }];
        let err = resolve(&manifests, &[], None).unwrap_err();
        assert!(matches!(
            err,
            ResolutionError::MissingCapability { capability, .. } if capability == "missing-cap".into()
        ));
    }

    #[test]
    fn selector_pins_provider() {
        let manifests = vec![
            manifest("provider-a", "1.0.0", &["compile"]),
            manifest("provider-b", "1.0.0", &["compile"]),
        ];
        let selectors = vec![CapabilitySelector {
            capability: "compile".into(),
            provider: "provider-b".into(),
        }];
        let resolved = resolve(&manifests, &selectors, None).unwrap();
        assert_eq!(
            resolved.provider_map.get(&"compile".into()),
            Some(&"provider-b".to_string())
        );
    }

    #[test]
    fn exclusive_capability_multiple_providers_fails() {
        let manifests = vec![
            PluginManifest {
                id: "a".into(),
                version: "1.0.0".into(),
                provides: vec!["x".into()],
                requires: Vec::new(),
                optional: Vec::new(),
                extends: Vec::new(),
                conflicts: Vec::new(),
                replaces: Vec::new(),
                targets: Vec::new(),
                exclusive: vec!["x".into()],
                metadata: serde_json::Value::default(),
            },
            PluginManifest {
                id: "b".into(),
                version: "1.0.0".into(),
                provides: vec!["x".into()],
                requires: Vec::new(),
                optional: Vec::new(),
                extends: Vec::new(),
                conflicts: Vec::new(),
                replaces: Vec::new(),
                targets: Vec::new(),
                exclusive: Vec::new(),
                metadata: serde_json::Value::default(),
            },
        ];
        let err = resolve(&manifests, &[], None).unwrap_err();
        assert!(matches!(
            err,
            ResolutionError::MultipleProvidersForExclusiveCapability { .. }
        ));
    }

    #[test]
    fn cycle_in_extends_graph_fails() {
        let manifests = vec![
            PluginManifest {
                id: "a".into(),
                version: "1.0.0".into(),
                provides: Vec::new(),
                requires: Vec::new(),
                optional: Vec::new(),
                extends: vec![ExtendsDeclaration {
                    plugin: "b".into(),
                    version: None,
                    extension_point: None,
                }],
                conflicts: Vec::new(),
                replaces: Vec::new(),
                targets: Vec::new(),
                exclusive: Vec::new(),
                metadata: serde_json::Value::default(),
            },
            PluginManifest {
                id: "b".into(),
                version: "1.0.0".into(),
                provides: Vec::new(),
                requires: Vec::new(),
                optional: Vec::new(),
                extends: vec![ExtendsDeclaration {
                    plugin: "a".into(),
                    version: None,
                    extension_point: None,
                }],
                conflicts: Vec::new(),
                replaces: Vec::new(),
                targets: Vec::new(),
                exclusive: Vec::new(),
                metadata: serde_json::Value::default(),
            },
        ];
        let err = resolve(&manifests, &[], None).unwrap_err();
        assert!(matches!(err, ResolutionError::CyclicGraph { .. }));
    }

    #[test]
    fn optional_missing_dependency_is_recorded() {
        let manifests = vec![PluginManifest {
            id: "main".into(),
            version: "1.0.0".into(),
            provides: Vec::new(),
            requires: Vec::new(),
            optional: vec![Dependency::plugin("extra", None)],
            extends: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
            targets: Vec::new(),
            exclusive: Vec::new(),
            metadata: serde_json::Value::default(),
        }];
        let resolved = resolve(&manifests, &[], None).unwrap();
        assert_eq!(resolved.missing_optional.len(), 1);
        assert!(
            matches!(resolved.missing_optional[0], Dependency::Plugin(PluginDependency { ref id, .. }) if id == "extra")
        );
    }

    #[test]
    fn picks_highest_version_for_shared_capability() {
        let manifests = vec![
            manifest("foo-1", "1.0.0", &["foo"]),
            manifest("foo-2", "1.1.0", &["foo"]),
        ];
        let resolved = resolve(&manifests, &[], None).unwrap();
        assert_eq!(
            resolved.provider_map.get(&"foo".into()),
            Some(&"foo-2".to_string())
        );
    }

    #[test]
    fn capability_version_constraint_accepts_satisfying() {
        let manifests = vec![
            PluginManifest {
                id: "main".into(),
                version: "1.0.0".into(),
                provides: Vec::new(),
                requires: vec![Dependency::capability("foo", Some("^1.0".into()))],
                optional: Vec::new(),
                extends: Vec::new(),
                conflicts: Vec::new(),
                replaces: Vec::new(),
                targets: Vec::new(),
                exclusive: Vec::new(),
                metadata: serde_json::Value::default(),
            },
            manifest("foo-0", "0.9.0", &["foo"]),
            manifest("foo-1", "1.1.0", &["foo"]),
        ];
        let resolved = resolve(&manifests, &[], None).unwrap();
        assert_eq!(
            resolved.provider_map.get(&"foo".into()),
            Some(&"foo-1".to_string())
        );
    }

    #[test]
    fn capability_version_constraint_rejects_unsatisfied() {
        let manifests = vec![
            PluginManifest {
                id: "main".into(),
                version: "1.0.0".into(),
                provides: Vec::new(),
                requires: vec![Dependency::capability("foo", Some("^1.0".into()))],
                optional: Vec::new(),
                extends: Vec::new(),
                conflicts: Vec::new(),
                replaces: Vec::new(),
                targets: Vec::new(),
                exclusive: Vec::new(),
                metadata: serde_json::Value::default(),
            },
            manifest("foo-0", "0.9.0", &["foo"]),
        ];
        let err = resolve(&manifests, &[], None).unwrap_err();
        assert!(matches!(err, ResolutionError::VersionMismatch { .. }));
    }

    #[test]
    fn same_id_with_different_versions_conflicts() {
        let manifests = vec![
            manifest("same", "1.0.0", &["foo"]),
            manifest("same", "1.1.0", &["foo"]),
        ];
        let err = resolve(&manifests, &[], None).unwrap_err();
        assert!(matches!(err, ResolutionError::Conflict { .. }));
    }

    #[test]
    fn selector_missing_provider_and_invalid_selector() {
        let manifests = vec![manifest("a", "1.0.0", &["x"])];
        let selectors = vec![CapabilitySelector {
            capability: "x".into(),
            provider: "missing".into(),
        }];
        let err = resolve(&manifests, &selectors, None).unwrap_err();
        assert!(
            matches!(err, ResolutionError::MissingPlugin { plugin, .. } if plugin == "missing")
        );

        let b = manifest("b", "1.0.0", &["x"]);
        let a = manifest("a", "1.0.0", &["y"]);
        let selectors = vec![CapabilitySelector {
            capability: "x".into(),
            provider: "a".into(),
        }];
        let err = resolve(&[a, b], &selectors, None).unwrap_err();
        assert!(matches!(err, ResolutionError::InvalidSelector { .. }));
    }

    fn main_plugin() -> PluginManifest {
        PluginManifest {
            id: "main".into(),
            version: "1.0.0".into(),
            provides: vec![],
            requires: vec![],
            optional: vec![],
            extends: vec![],
            conflicts: vec![],
            replaces: vec![],
            targets: vec![],
            exclusive: vec![],
            metadata: serde_json::Value::Null,
        }
    }

    #[test]
    fn required_plugin_dependency_and_version_mismatch() {
        let mut main = main_plugin();
        main.requires = vec![Dependency::plugin("base", None)];
        let base = manifest("base", "1.0.0", &[]);

        let resolved = resolve(&[main.clone(), base], &[], None).unwrap();
        assert!(resolved.selected_plugins.contains(&"main".into()));

        main.requires = vec![Dependency::plugin("base", Some("^2.0".into()))];
        let base = manifest("base", "1.0.0", &[]);
        let err = resolve(&[main, base], &[], None).unwrap_err();
        assert!(matches!(err, ResolutionError::VersionMismatch { .. }));

        let mut main = main_plugin();
        main.requires = vec![Dependency::plugin("missing", None)];
        let err = resolve(&[main], &[], None).unwrap_err();
        assert!(
            matches!(err, ResolutionError::MissingPlugin { plugin, .. } if plugin == "missing")
        );
    }

    #[test]
    fn optional_dependencies_are_recorded_or_resolved() {
        let mut main = main_plugin();
        main.optional = vec![
            Dependency::capability("foo", Some("^1.0".into())),
            Dependency::plugin("bar", Some(">=2.0".into())),
            Dependency::capability("missing-cap", None),
            Dependency::plugin("missing-plugin", None),
        ];
        let foo = manifest("foo", "1.1.0", &["foo"]);
        let bar = manifest("bar", "2.0.0", &[]);

        let resolved = resolve(&[main, foo, bar], &[], None).unwrap();
        assert_eq!(resolved.missing_optional.len(), 2);
    }

    #[test]
    fn conflict_by_capability_and_missing_extendee() {
        let a = PluginManifest {
            id: "a".into(),
            version: "1.0.0".into(),
            provides: vec!["x".into()],
            requires: vec![],
            optional: vec![],
            extends: vec![],
            conflicts: vec![ConflictDeclaration {
                plugin: None,
                capability: Some("y".into()),
                reason: None,
            }],
            replaces: vec![],
            targets: vec![],
            exclusive: vec![],
            metadata: serde_json::Value::Null,
        };
        let b = manifest("b", "1.0.0", &["y"]);
        let err = resolve(&[a, b], &[], None).unwrap_err();
        assert!(matches!(err, ResolutionError::Conflict { plugin_a, .. } if plugin_a == "a"));

        let a = PluginManifest {
            id: "a".into(),
            version: "1.0.0".into(),
            provides: vec![],
            requires: vec![],
            optional: vec![],
            extends: vec![ExtendsDeclaration {
                plugin: "missing".into(),
                version: None,
                extension_point: None,
            }],
            conflicts: vec![],
            replaces: vec![],
            targets: vec![],
            exclusive: vec![],
            metadata: serde_json::Value::Null,
        };
        let err = resolve(&[a], &[], None).unwrap_err();
        assert!(matches!(err, ResolutionError::MissingPlugin { .. }));
    }

    #[test]
    fn target_report_filled_for_requested_target() {
        let a = PluginManifest {
            id: "a".into(),
            version: "1.0.0".into(),
            provides: vec![],
            requires: vec![],
            optional: vec![],
            extends: vec![],
            conflicts: vec![],
            replaces: vec![],
            targets: vec![TargetCompatibility {
                target: "wasm".into(),
                level: TargetSupportLevel::Checked,
            }],
            exclusive: vec![],
            metadata: serde_json::Value::Null,
        };
        let b = manifest("b", "1.0.0", &[]);

        let resolved = resolve(&[a, b], &[], Some("wasm")).unwrap();
        assert_eq!(
            resolved.target_report.get("a"),
            Some(&TargetSupportLevel::Checked)
        );
        assert_eq!(
            resolved.target_report.get("b"),
            Some(&TargetSupportLevel::Unsupported)
        );
    }

    #[test]
    fn version_helpers() {
        assert_eq!(version_cmp("1.0.0", "2.0.0"), std::cmp::Ordering::Less);
        assert_eq!(version_cmp("1.10", "1.2"), std::cmp::Ordering::Greater);
        assert!(version_satisfies("1.0.0", "1.0.0"));
        assert!(version_satisfies("^1.0", "1.2.0"));
        assert!(!version_satisfies("^2.0", "1.0.0"));
        assert_eq!(normalize_version_req("1"), Some("=1.0.0".into()));
        assert_eq!(normalize_version_req(">=1.2"), Some(">=1.2.0".into()));
        assert_eq!(normalize_version_req(""), None);
    }

    #[test]
    fn init_order_respects_plugin_dependency_and_extends() {
        let base = manifest("base", "1.0.0", &[]);
        let mut ext = main_plugin();
        ext.id = "ext".into();
        ext.requires = vec![Dependency::plugin("base", None)];
        ext.extends = vec![ExtendsDeclaration {
            plugin: "base".into(),
            version: None,
            extension_point: None,
        }];

        let resolved = resolve(&[ext, base], &[], None).unwrap();
        assert!(
            resolved
                .initialization_order
                .iter()
                .position(|id| id == "base")
                .unwrap()
                < resolved
                    .initialization_order
                    .iter()
                    .position(|id| id == "ext")
                    .unwrap()
        );
    }

    #[test]
    fn conflict_by_plugin_declaration() {
        let a = PluginManifest {
            id: "a".into(),
            version: "1.0.0".into(),
            provides: vec![],
            requires: vec![],
            optional: vec![],
            extends: vec![],
            conflicts: vec![ConflictDeclaration {
                plugin: Some("b".into()),
                capability: None,
                reason: Some("incompatible".into()),
            }],
            replaces: vec![],
            targets: vec![],
            exclusive: vec![],
            metadata: serde_json::Value::Null,
        };
        let b = manifest("b", "1.0.0", &[]);
        let err = resolve(&[a, b], &[], None).unwrap_err();
        assert!(matches!(err, ResolutionError::Conflict { plugin_a, .. } if plugin_a == "a"));
    }

    #[test]
    fn cycle_via_capability_dependencies() {
        let a = PluginManifest {
            id: "a".into(),
            version: "1.0.0".into(),
            provides: vec!["x".into()],
            requires: vec![Dependency::capability("y", None)],
            optional: vec![],
            extends: vec![],
            conflicts: vec![],
            replaces: vec![],
            targets: vec![],
            exclusive: vec![],
            metadata: serde_json::Value::Null,
        };
        let b = PluginManifest {
            id: "b".into(),
            version: "1.0.0".into(),
            provides: vec!["y".into()],
            requires: vec![Dependency::capability("x", None)],
            optional: vec![],
            extends: vec![],
            conflicts: vec![],
            replaces: vec![],
            targets: vec![],
            exclusive: vec![],
            metadata: serde_json::Value::Null,
        };
        let err = resolve(&[a, b], &[], None).unwrap_err();
        assert!(matches!(err, ResolutionError::CyclicGraph { .. }));
    }

    #[test]
    fn version_satisfies_fallback_exact() {
        assert!(version_satisfies("1.x", "1.x"));
        assert!(!version_satisfies("1.x", "1.y"));
    }
}
