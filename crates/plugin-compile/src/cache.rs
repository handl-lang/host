//! Incremental artifact cache for per-module compilation (ADR-0114 Phase 12).

use std::collections::HashMap;

use crate::{Artifact, CompilerConfig, CompilerInput, CompilerOutput};

/// Cache key for a single `(input, target)` pair.
pub fn cache_key(input: &CompilerInput, config: &dyn CompilerConfig) -> String {
    format!(
        "compile:{}:{}:{}",
        config.target(),
        input.root.0,
        input.source
    )
}

/// A cached `CompilerOutput` reconstructed from a single stored `Artifact`.
#[derive(Debug, Clone)]
pub struct CachedOutput {
    target: String,
    artifact: Artifact,
}

impl CachedOutput {
    /// Construct a `CachedOutput` from `target` and `artifact`.
    pub fn new(target: impl Into<String>, artifact: Artifact) -> Self {
        Self {
            target: target.into(),
            artifact,
        }
    }
}

impl CompilerOutput for CachedOutput {
    fn target(&self) -> &str {
        &self.target
    }

    fn artifacts(&self) -> &[Artifact] {
        std::slice::from_ref(&self.artifact)
    }
}

/// Incremental artifact cache used by [`CompilerRegistry::compile_many_cached`].
pub trait ArtifactCache: Send + Sync {
    /// Look up an artifact by `key`.
    fn get(&self, key: &str) -> Option<Artifact>;

    /// Store `artifact` under `key`.
    fn put(&mut self, key: &str, artifact: Artifact);
}

/// Simple in-memory `ArtifactCache` backed by a `HashMap`.
#[derive(Debug, Default, Clone)]
pub struct MemoryArtifactCache {
    entries: HashMap<String, Artifact>,
}

impl MemoryArtifactCache {
    /// Create an empty in-memory cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl ArtifactCache for MemoryArtifactCache {
    fn get(&self, key: &str) -> Option<Artifact> {
        self.entries.get(key).cloned()
    }

    fn put(&mut self, key: &str, artifact: Artifact) {
        self.entries.insert(key.to_string(), artifact);
    }
}
