use std::collections::HashMap;

use rayon::prelude::*;

use crate::cache::{CachedOutput, cache_key};
use crate::{ArtifactCache, CompileError, Compiler, CompilerConfig, CompilerInput, CompilerOutput};

/// A registry of [`Compiler`]s keyed by target name.
#[derive(Debug, Default)]
pub struct CompilerRegistry {
    compilers: HashMap<String, Box<dyn Compiler>>,
}

impl CompilerRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            compilers: HashMap::new(),
        }
    }

    /// Register `compiler` under its target name.
    pub fn register(&mut self, compiler: Box<dyn Compiler>) {
        self.compilers
            .insert(compiler.target().to_string(), compiler);
    }

    /// Look up a compiler for `target`.
    pub fn get(&self, target: &str) -> Option<&dyn Compiler> {
        self.compilers.get(target).map(|b| b.as_ref())
    }

    /// All registered target names.
    pub fn targets(&self) -> Vec<&str> {
        self.compilers.keys().map(|s| s.as_str()).collect()
    }

    /// Compile `inputs` in parallel using `config`.
    pub fn compile_many(
        &self,
        inputs: Vec<CompilerInput>,
        config: &dyn CompilerConfig,
    ) -> Vec<Result<Box<dyn CompilerOutput>, CompileError>> {
        let compiler = match self.get(config.target()) {
            Some(c) => c,
            None => {
                return inputs
                    .into_iter()
                    .map(|_| Err(CompileError::target(config.target())))
                    .collect();
            }
        };
        inputs
            .into_par_iter()
            .map(|input| compiler.compile(&input, config))
            .collect()
    }

    /// Compile `inputs` in parallel, reading and writing `cache`.
    pub fn compile_many_cached(
        &self,
        inputs: Vec<CompilerInput>,
        config: &dyn CompilerConfig,
        cache: &mut dyn ArtifactCache,
    ) -> Vec<Result<Box<dyn CompilerOutput>, CompileError>> {
        let compiler = match self.get(config.target()) {
            Some(c) => c,
            None => {
                return inputs
                    .into_iter()
                    .map(|_| Err(CompileError::target(config.target())))
                    .collect();
            }
        };

        let mut results: Vec<Option<Result<Box<dyn CompilerOutput>, CompileError>>> =
            (0..inputs.len()).map(|_| None).collect();
        let mut to_compile: Vec<(usize, String, CompilerInput)> = Vec::with_capacity(inputs.len());

        for (i, input) in inputs.into_iter().enumerate() {
            let key = cache_key(&input, config);
            match cache.get(&key) {
                Some(artifact) => {
                    results[i] = Some(Ok(Box::new(CachedOutput::new(config.target(), artifact))));
                }
                None => to_compile.push((i, key, input)),
            }
        }

        let compiled = to_compile
            .into_par_iter()
            .map(|(i, key, input)| (i, key, compiler.compile(&input, config)))
            .collect::<Vec<_>>();

        for (i, key, result) in compiled {
            if let Ok(output) = &result
                && let Some(artifact) = output.artifacts().first()
            {
                cache.put(&key, artifact.clone());
            }
            results[i] = Some(result);
        }

        results.into_iter().map(|r| r.unwrap()).collect()
    }
}
