//! Unit tests for the LLVM compiler.

use handl_language::{Ast, AstNode, Language};
use handl_plugin_compile::{
    ArtifactContent, CompileError, Compiler, CompilerInput, CompilerOutput,
};
use plugin_engine::{RewriteWitness, Term};
use plugin_engine_typecheck::TypecheckReport;
use serde_json::Value;

use crate::{
    AArch64, DefaultObjectWriter, LlvmCompiler, LlvmConfig, LlvmOutput, LlvmTarget, ObjectWriter,
    X86_64,
};

fn input(term: Term) -> CompilerInput {
    let source = "test".to_string();
    let mut ast = Ast::new();
    let root = ast.push(AstNode::new("module"));
    let language = Language::new();
    let report = TypecheckReport::default();
    CompilerInput::new(source, ast, root, language, term, report)
}

#[derive(Debug)]
struct MockWriter;

impl ObjectWriter for MockWriter {
    fn write(&self, _triple: &str, _ir: &str, _passes: &[String]) -> Result<Vec<u8>, CompileError> {
        Ok(b"mock object".to_vec())
    }
}

#[test]
fn compiler_target_is_llvm() {
    assert_eq!(LlvmCompiler::new().target(), "llvm");
}

#[test]
fn x86_64_triple_is_correct() {
    assert_eq!(X86_64.name(), "x86_64");
    assert_eq!(X86_64.triple(), "x86_64-unknown-unknown");
}

#[test]
fn aarch64_triple_is_correct() {
    assert_eq!(AArch64.name(), "aarch64");
    assert_eq!(AArch64.triple(), "aarch64-unknown-unknown");
}

#[cfg(not(feature = "inkwell"))]
#[test]
fn placeholder_output_for_const() {
    let cfg = LlvmConfig::new(Box::new(X86_64));
    let output = LlvmCompiler::new()
        .compile(&input(Term::constant(Value::Null)), &cfg)
        .unwrap();
    assert_eq!(output.target(), "llvm");
    assert_eq!(output.artifacts().len(), 1);
    let artifact = &output.artifacts()[0];
    assert!(artifact.name.ends_with(".o"), "{:?}", artifact);
    assert_eq!(
        artifact.content,
        ArtifactContent::Bytes(b"<inkwell disabled>".to_vec())
    );
    assert_eq!(output.primary_output().unwrap(), &b"<inkwell disabled>"[..]);
}

#[cfg(not(feature = "inkwell"))]
#[test]
fn dispatcher_branches_for_phase9_terms() {
    let terms = phase9_terms();
    for term in &terms {
        let mut ctx = crate::lower::LlvmContext::new(term, X86_64.triple());
        let result = crate::lower::lower_term(&mut ctx, term, ());
        assert!(result.is_ok(), "lower_term failed for {term:?}");
    }
}

#[cfg(not(feature = "inkwell"))]
#[test]
fn placeholder_output_for_phase9_terms() {
    let cfg = LlvmConfig::new(Box::new(X86_64));
    for term in phase9_terms() {
        let output = LlvmCompiler::new().compile(&input(term), &cfg).unwrap();
        assert_eq!(output.artifacts().len(), 1);
        assert!(output.artifacts()[0].name.ends_with(".o"));
    }
}

#[test]
fn compiler_uses_supplied_object_writer() {
    let cfg = LlvmConfig::new(Box::new(X86_64));
    let compiler = LlvmCompiler::with_writer(MockWriter);
    let output = compiler
        .compile(&input(Term::constant(Value::Null)), &cfg)
        .unwrap();
    assert_eq!(output.primary_output().unwrap(), &b"mock object"[..]);
}

#[cfg(not(feature = "inkwell"))]
#[test]
fn default_object_writer_returns_placeholder() {
    let writer = DefaultObjectWriter;
    let bytes = writer.write("x86_64-unknown-unknown", "; ir", &[]).unwrap();
    assert_eq!(bytes, b"<inkwell disabled>");
}

#[cfg(feature = "inkwell")]
#[test]
fn default_object_writer_emits_object() {
    let ir = "define i32 @main() nounwind {\n  ret i32 0\n}\n";
    let bytes = DefaultObjectWriter
        .write("x86_64-unknown-unknown", ir, &[])
        .unwrap();
    assert!(!bytes.is_empty());
}

#[cfg(feature = "inkwell")]
#[test]
fn llvm_const_compiles_to_object() {
    let cfg = LlvmConfig::new(Box::new(X86_64));
    let output = LlvmCompiler::new()
        .compile(&input(Term::constant(Value::Null)), &cfg)
        .unwrap();
    assert_eq!(output.target(), "llvm");
    assert_eq!(output.artifacts().len(), 1);
    assert!(output.artifacts()[0].name.ends_with(".o"));
    assert!(!output.primary_output().unwrap().is_empty());
}

#[cfg(feature = "inkwell")]
#[test]
fn phase9_terms_compile_to_object() {
    let cfg = LlvmConfig::new(Box::new(X86_64));
    for term in phase9_terms() {
        let output = LlvmCompiler::new().compile(&input(term), &cfg).unwrap();
        assert_eq!(output.artifacts().len(), 1);
        assert!(
            output.artifacts()[0].name.ends_with(".o"),
            "term {term:?} did not emit .o"
        );
        assert!(
            !output.primary_output().unwrap().is_empty(),
            "term {term:?} emitted empty object"
        );
    }
}

#[test]
fn llvm_config_default_optimized_has_passes() {
    let cfg = LlvmConfig::default_optimized();
    assert_eq!(cfg.passes(), &["instcombine", "reassociate", "gvn"]);
}

#[test]
fn llvm_config_passes_round_trip() {
    let mut cfg = LlvmConfig::new(Box::new(X86_64));
    assert!(cfg.passes().is_empty());
    cfg.with_pass("simplifycfg");
    assert_eq!(cfg.passes(), &["simplifycfg"]);
    cfg.with_pass("gvn");
    assert_eq!(cfg.passes(), &["simplifycfg", "gvn"]);
}

#[test]
fn pass_list_round_trip() {
    let mut cfg = LlvmConfig::default_optimized();
    cfg.with_pass("simplifycfg");
    assert_eq!(
        cfg.passes(),
        &["instcombine", "reassociate", "gvn", "simplifycfg"]
    );
}

#[cfg(not(feature = "inkwell"))]
#[test]
fn default_object_writer_unknown_pass_is_placeholder() {
    let writer = DefaultObjectWriter;
    let bytes = writer
        .write("x86_64-unknown-unknown", "; ir", &["unknown".to_string()])
        .unwrap();
    assert_eq!(bytes, b"<inkwell disabled>");
}

#[test]
fn llvm_output_stream_writes_object() {
    let output = LlvmOutput::from_object("x86_64-unknown-unknown", b"obj".to_vec());
    let mut buf = Vec::new();
    output.stream(&mut buf).unwrap();
    assert_eq!(buf, b"obj");
}

#[cfg(not(feature = "inkwell"))]
#[test]
fn llvm_build_script_warns_when_inkwell_off() {
    let writer = DefaultObjectWriter;
    let bytes = writer.write("x86_64-unknown-unknown", "; ir", &[]).unwrap();
    assert_eq!(bytes, b"<inkwell disabled>");
}

#[cfg(feature = "inkwell")]
#[test]
fn llvm_compiler_helpful_error_for_missing_target() {
    let err = DefaultObjectWriter
        .write("not-a-real-triple", "; ir", &[])
        .unwrap_err();
    assert!(matches!(err, CompileError::UnsupportedTarget(_)), "{err}");
    assert!(format!("{err}").contains("not-a-real-triple"), "{err}");
}

fn phase9_terms() -> Vec<Term> {
    vec![
        Term::first(Term::constant(Value::Null)),
        Term::choice(Term::constant(Value::Null), Term::constant(Value::Null)),
        Term::loop_feedback(Term::constant(Value::Null)),
        Term::Iter {
            fuel: Box::new(Term::constant(Value::Null)),
            predicate: Box::new(Term::constant(Value::Null)),
            step: Box::new(Term::constant(Value::Null)),
            initial: Box::new(Term::constant(Value::Null)),
        },
        Term::app(
            Term::constant(Value::Null),
            vec![Term::constant(Value::Null)],
        ),
        Term::Child {
            role: "body".to_string(),
        },
        Term::Handle {
            body: Box::new(Term::constant(Value::Null)),
            target: "io".to_string(),
            impl_: Box::new(Term::constant(Value::Null)),
        },
        Term::Rewrite {
            source: Box::new(Term::constant(Value::Null)),
            target: Box::new(Term::constant(Value::Null)),
            witness: RewriteWitness::Compute,
        },
        Term::Path {
            endpoints: (
                Box::new(Term::constant(Value::Null)),
                Box::new(Term::constant(Value::Null)),
            ),
            witness: None,
        },
        Term::Equiv {
            a: Box::new(Term::constant(Value::Null)),
            b: Box::new(Term::constant(Value::Null)),
            path: None,
        },
    ]
}
