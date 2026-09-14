use std::path::PathBuf;

use handl_language::{Ast, AstNode, AstRef, Language};
use plugin_engine::Term;
use plugin_engine_typecheck::TypecheckReport;
use serde_json::Value;

use crate::{
    Artifact, ArtifactContent, CompileError, Compiler, CompilerConfig, CompilerInput,
    CompilerOutput, CompilerRegistry, DebugInfo, MemoryArtifactCache, Null, SourceMap, V3,
};

#[derive(Debug)]
struct MockConfig {
    target: String,
    debug: bool,
}

impl MockConfig {
    fn new(target: &str) -> Self {
        Self {
            target: target.to_string(),
            debug: false,
        }
    }
}

impl CompilerConfig for MockConfig {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn target(&self) -> &str {
        &self.target
    }

    fn debug(&self) -> bool {
        self.debug
    }
}

#[derive(Debug, Default)]
struct MockOutput {
    target: String,
    artifacts: Vec<Artifact>,
}

impl MockOutput {
    fn new(target: &str) -> Self {
        Self {
            target: target.to_string(),
            artifacts: vec![Artifact::new("out.txt", "compiled")],
        }
    }
}

impl CompilerOutput for MockOutput {
    fn target(&self) -> &str {
        &self.target
    }

    fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }
}

#[derive(Debug)]
struct MockCompiler {
    target: &'static str,
}

impl Compiler for MockCompiler {
    fn compile(
        &self,
        _input: &CompilerInput,
        config: &dyn CompilerConfig,
    ) -> Result<Box<dyn CompilerOutput>, CompileError> {
        if config.target() != self.target {
            return Err(CompileError::target(config.target()));
        }
        Ok(Box::new(MockOutput::new(self.target)))
    }

    fn target(&self) -> &str {
        self.target
    }
}

fn minimal_input() -> CompilerInput {
    let source = "main = 1".to_string();
    let mut ast = Ast::new();
    let root = ast.push(AstNode::new("module"));
    let language = Language::new();
    let term = Term::const_value(Value::Null);
    let report = TypecheckReport::default();
    CompilerInput::new(source, ast, root, language, term, report)
}

#[test]
fn compile_error_display_variants() {
    assert_eq!(
        format!("{}", CompileError::Io("disk full".to_string())),
        "io error: disk full"
    );
    assert_eq!(
        format!("{}", CompileError::target("mips")),
        "unsupported target: mips"
    );
    assert_eq!(format!("{}", CompileError::other("misc")), "misc");
    assert!(format!("{}", CompileError::Lower("x".to_string())).contains("lower"));
    assert!(format!("{}", CompileError::Emit("x".to_string())).contains("emit"));
    assert!(format!("{}", CompileError::Link("x".to_string())).contains("link"));
    assert!(format!("{}", CompileError::InvalidInput("x".to_string())).contains("invalid"));
}

#[test]
fn artifact_construction() {
    let a = Artifact::new("a.c", b"hello".to_vec());
    assert_eq!(a.name, "a.c");
    assert!(a.path.is_none());
    assert_eq!(a.content, ArtifactContent::Bytes(b"hello".to_vec()));

    let b = Artifact::new("b.js", "code").with_path(PathBuf::from("/tmp/b.js"));
    assert_eq!(b.content, ArtifactContent::String("code".to_string()));
    assert_eq!(b.path, Some(PathBuf::from("/tmp/b.js")));

    let c: ArtifactContent = "text".into();
    assert_eq!(c, ArtifactContent::String("text".to_string()));
}

#[test]
fn mock_compiler_and_registry() {
    let mut registry = CompilerRegistry::new();
    registry.register(Box::new(MockCompiler { target: "mock" }));

    assert!(registry.get("missing").is_none());
    let compiler = registry.get("mock").expect("registered mock");
    assert_eq!(compiler.target(), "mock");

    let targets = registry.targets();
    assert_eq!(targets.len(), 1);
    assert!(targets.contains(&"mock"));

    let input = minimal_input();
    let config = MockConfig::new("mock");
    let output = compiler.compile(&input, &config).expect("compile succeeds");
    assert_eq!(output.target(), "mock");
    assert_eq!(output.artifacts().len(), 1);
    assert_eq!(output.primary_output(), Some(b"compiled".as_slice()));
}

#[test]
fn compiler_rejects_wrong_target() {
    let compiler = MockCompiler { target: "c" };
    let input = minimal_input();
    let config = MockConfig::new("js");
    let err = compiler.compile(&input, &config).unwrap_err();
    assert!(matches!(err, CompileError::UnsupportedTarget(_)));
    assert!(format!("{err}").contains("js"));
}

#[test]
fn compiler_input_construction() {
    let input = minimal_input();
    assert_eq!(input.source, "main = 1");
    assert_eq!(input.root, AstRef::new(0));
    assert_eq!(input.language, Language::new());
    assert!(input.report.is_clean());
}

#[test]
fn source_map_placeholder() {
    let map = V3::new();
    let out = map.generate().expect("generate succeeds");
    assert!(out.contains("version"));
}

#[test]
fn debug_info_placeholder() {
    let info = Null;
    assert!(info.format().is_empty());
}

#[test]
fn artifact_stream_writes_content() {
    let artifact = Artifact::new("x.js", "hello");
    let mut buf = Vec::new();
    artifact.stream(&mut buf).unwrap();
    assert_eq!(buf, b"hello");
}

#[test]
fn compiler_is_send_sync() {
    let compiler = MockCompiler { target: "mock" };
    let input = minimal_input();
    let config = MockConfig::new("mock");
    let (a, b) = rayon::join(
        || compiler.compile(&input, &config),
        || compiler.compile(&input, &config),
    );
    assert!(a.is_ok());
    assert!(b.is_ok());
}

#[test]
fn compile_many_returns_ordered_results() {
    let mut registry = CompilerRegistry::new();
    registry.register(Box::new(MockCompiler { target: "c" }));
    registry.register(Box::new(MockCompiler { target: "js" }));
    registry.register(Box::new(MockCompiler { target: "llvm" }));

    for target in ["c", "js", "llvm"] {
        let inputs = vec![minimal_input(), minimal_input(), minimal_input()];
        let config = MockConfig::new(target);
        let results = registry.compile_many(inputs, &config);
        assert_eq!(results.len(), 3, "target {target}");
        for (i, result) in results.iter().enumerate() {
            let output = result
                .as_ref()
                .unwrap_or_else(|_| panic!("compile_many failed for {target} at {i}"));
            assert_eq!(output.target(), target);
            assert_eq!(output.artifacts().len(), 1);
        }
    }
}

#[test]
fn compile_many_cached_returns_cached_artifacts() {
    let mut registry = CompilerRegistry::new();
    registry.register(Box::new(MockCompiler { target: "mock" }));
    let mut cache = MemoryArtifactCache::new();
    let config = MockConfig::new("mock");

    let inputs = vec![minimal_input(), minimal_input()];
    let first = registry.compile_many_cached(inputs, &config, &mut cache);
    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|r| r.is_ok()));

    let inputs2 = vec![minimal_input(), minimal_input()];
    let second = registry.compile_many_cached(inputs2, &config, &mut cache);
    assert_eq!(second.len(), 2);
    assert!(second.iter().all(|r| r.is_ok()));
    assert_eq!(
        first[0].as_ref().unwrap().artifacts(),
        second[0].as_ref().unwrap().artifacts()
    );
}
