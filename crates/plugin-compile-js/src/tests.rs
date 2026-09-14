use handl_language::{Ast, AstNode, Language};
use handl_plugin_compile::{ArtifactContent, Compiler, CompilerInput, CompilerOutput};
use plugin_engine::term::RewriteWitness;
use plugin_engine::{ArrSource, Term};
use plugin_engine_typecheck::TypecheckReport;
use rand::Rng;
use serde_json::{Number, Value, json};

use crate::security::{is_safe_for_inlining, validate_foreign_descriptor};
use crate::{
    Browser, Bun, Iife, Inline, JsCompiler, JsConfig, JsOutput, JsTarget, compile_to_string,
};

fn input(term: Term) -> CompilerInput {
    let source = "test".to_string();
    let mut ast = Ast::new();
    let root = ast.push(AstNode::new("module"));
    let language = Language::new();
    let report = TypecheckReport::default();
    CompilerInput::new(source, ast, root, language, term, report)
}

#[test]
fn compiler_target_is_js() {
    assert_eq!(JsCompiler.target(), "js");
}

#[test]
fn const_null() {
    let js = compile_to_string(&input(Term::constant(Value::Null)), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("export default null;"), "{js}");
}

#[test]
fn const_bool() {
    let js = compile_to_string(&input(Term::constant(true)), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("export default true;"), "{js}");
}

#[test]
fn const_int() {
    let js = compile_to_string(
        &input(Term::constant(Value::Number(Number::from(42)))),
        &JsConfig::esm_deno(),
    )
    .unwrap();
    assert!(js.contains("export default 42;"), "{js}");
}

#[test]
fn const_float() {
    let js = compile_to_string(
        &input(Term::constant(Value::Number(
            Number::from_f64(3.14).unwrap(),
        ))),
        &JsConfig::esm_deno(),
    )
    .unwrap();
    assert!(js.contains("export default 3.14;"), "{js}");
}

#[test]
fn const_string() {
    let js = compile_to_string(
        &input(Term::constant(Value::String("hello".to_string()))),
        &JsConfig::esm_deno(),
    )
    .unwrap();
    assert!(js.contains(r#"export default "hello";"#), "{js}");
}

#[test]
fn arr_constant_value() {
    let js = compile_to_string(
        &input(Term::const_value(Value::Number(Number::from(42)))),
        &JsConfig::esm_deno(),
    )
    .unwrap();
    assert!(js.contains("=> 42"), "{js}");
    assert!(js.contains("__in"), "{js}");
}

#[test]
fn op_add() {
    let term = Term::op_native(
        "add",
        vec![
            Term::const_value(Value::Number(Number::from(1))),
            Term::const_value(Value::Number(Number::from(2))),
        ],
    );
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("1 + 2"), "{js}");
}

#[test]
fn op_proj_env() {
    let term = Term::op_native_lit("proj_env", vec![], Value::String("x".to_string()));
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("__handl_rt.projEnv"), "{js}");
    assert!(js.contains(r#""x""#), "{js}");
}

#[test]
fn seq_constant_arrows() {
    let term = Term::seq(
        Term::const_value(Value::Number(Number::from(1))),
        Term::const_value(Value::Number(Number::from(2))),
    );
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("=>"), "{js}");
    assert!(js.contains("2"), "{js}");
}

#[test]
fn esm_wrapping() {
    let js = compile_to_string(
        &input(Term::const_value(Value::Number(Number::from(1)))),
        &JsConfig::esm_deno(),
    )
    .unwrap();
    assert!(js.contains("export default"), "{js}");
    assert!(js.contains("const __handl_rt"), "{js}");
}

#[test]
fn commonjs_wrapping() {
    let js = compile_to_string(
        &input(Term::const_value(Value::Number(Number::from(1)))),
        &JsConfig::cjs_node(),
    )
    .unwrap();
    assert!(js.contains("module.exports ="), "{js}");
    assert!(js.contains("var __handl_rt"), "{js}");
}

#[test]
fn first_on_pair() {
    let term = Term::first(Term::op_native_lit(
        "proj_env",
        vec![],
        Value::String("x".to_string()),
    ));
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("__handl_rt.fst"), "{js}");
    assert!(js.contains("__handl_rt.snd"), "{js}");
    assert!(js.contains("__handl_rt.mkPair"), "{js}");
    assert!(js.contains("=>"), "{js}");
}

#[test]
fn choice_on_either() {
    let term = Term::choice(
        Term::op_native_lit("proj_env", vec![], Value::String("a".to_string())),
        Term::op_native_lit("proj_env", vec![], Value::String("b".to_string())),
    );
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("__handl_rt.isLeft"), "{js}");
    assert!(js.contains("__handl_rt.leftValue"), "{js}");
    assert!(js.contains("__handl_rt.rightValue"), "{js}");
    assert!(js.contains("if ("), "{js}");
}

#[test]
fn loop_emits_while() {
    let term = Term::loop_feedback(Term::const_value(Value::Number(Number::from(42))));
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("while ("), "{js}");
    assert!(js.contains("__handl_rt.isLeft"), "{js}");
    assert!(js.contains("__handl_rt.isRight"), "{js}");
    assert!(js.contains("__handl_rt.isPair"), "{js}");
    assert!(js.contains("100000"), "{js}");
}

#[test]
fn iter_emits_bounded_loop() {
    let term = Term::Iter {
        fuel: Box::new(Term::const_value(Value::Number(Number::from(10)))),
        predicate: Box::new(Term::const_value(Value::Bool(true))),
        step: Box::new(Term::const_value(Value::Number(Number::from(1)))),
        initial: Box::new(Term::const_value(Value::Number(Number::from(0)))),
    };
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("while ("), "{js}");
    assert!(js.contains("__state"), "{js}");
    assert!(js.contains("__cnt"), "{js}");
    assert!(js.contains("return __state"), "{js}");
}

#[test]
fn app_emits_call() {
    let term = Term::app(
        Term::const_value(Value::String("fn".to_string())),
        vec![Term::const_value(Value::Number(Number::from(1)))],
    );
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("\"fn\"(1)"), "{js}");
    assert!(js.contains("__in"), "{js}");
}

#[test]
fn combined_first_then_loop() {
    let term = Term::seq(
        Term::first(Term::const_value(Value::Number(Number::from(1)))),
        Term::loop_feedback(Term::const_value(Value::Number(Number::from(2)))),
    );
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("__handl_rt.fst"), "{js}");
    assert!(js.contains("__handl_rt.mkPair"), "{js}");
    assert!(js.contains("while ("), "{js}");
}

#[test]
fn combined_first_then_choice() {
    let term = Term::seq(
        Term::first(Term::op_native_lit(
            "proj_env",
            vec![],
            Value::String("x".to_string()),
        )),
        Term::choice(
            Term::op_native_lit("proj_env", vec![], Value::String("l".to_string())),
            Term::op_native_lit("proj_env", vec![], Value::String("r".to_string())),
        ),
    );
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("__handl_rt.fst"), "{js}");
    assert!(js.contains("__handl_rt.isLeft"), "{js}");
    assert!(js.contains("__handl_rt.mkPair"), "{js}");
}

#[test]
fn child_lowering() {
    let term = Term::Child {
        role: "body".to_string(),
    };
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("__handl_rt.child"), "{js}");
    assert!(js.contains(r#""body""#), "{js}");
}

#[test]
fn handle_lowering() {
    let term = Term::handle(
        Term::const_value(Value::Number(Number::from(1))),
        "cap",
        Term::const_value(Value::Number(Number::from(2))),
    );
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("__handl_rt.handle"), "{js}");
    assert!(js.contains(r#""cap""#), "{js}");
}

#[test]
fn rewrite_lowering() {
    let term = Term::Rewrite {
        source: Box::new(Term::const_value(Value::Number(Number::from(1)))),
        target: Box::new(Term::const_value(Value::Number(Number::from(2)))),
        witness: RewriteWitness::Named("foo".to_string()),
    };
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("__handl_rt.rewrite"), "{js}");
    assert!(js.contains(r#""foo""#), "{js}");
}

#[test]
fn path_lowering() {
    let term = Term::Path {
        endpoints: (
            Box::new(Term::const_value(Value::Number(Number::from(1)))),
            Box::new(Term::const_value(Value::Number(Number::from(2)))),
        ),
        witness: None,
    };
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("__handl_rt.path"), "{js}");
}

#[test]
fn equiv_lowering() {
    let term = Term::Equiv {
        a: Box::new(Term::const_value(Value::Number(Number::from(1)))),
        b: Box::new(Term::const_value(Value::Number(Number::from(2)))),
        path: None,
    };
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains("__handl_rt.equiv"), "{js}");
}

#[test]
fn bun_target_name() {
    assert_eq!(Bun.name(), "bun");
}

#[test]
fn browser_target_name() {
    assert_eq!(Browser.name(), "browser");
}

#[test]
fn iife_wrapping() {
    let config = JsConfig {
        target: Box::new(Browser),
        format: Box::new(Iife),
        debug: false,
        no_inline_foreign: false,
        source_map: false,
    };
    let js = compile_to_string(
        &input(Term::const_value(Value::Number(Number::from(42)))),
        &config,
    )
    .unwrap();
    assert!(js.starts_with("(function"), "{js}");
    assert!(js.contains("return __in => 42"), "{js}");
    assert!(js.contains("const __handl_rt"), "{js}");
}

#[test]
fn inline_format() {
    let config = JsConfig {
        target: Box::new(Browser),
        format: Box::new(Inline),
        debug: false,
        no_inline_foreign: false,
        source_map: false,
    };
    let js = compile_to_string(
        &input(Term::const_value(Value::Number(Number::from(42)))),
        &config,
    )
    .unwrap();
    assert!(!js.contains("export"), "{js}");
    assert!(!js.contains("module.exports"), "{js}");
    assert!(js.contains("42"), "{js}");
}

fn artifact_to_string(content: &ArtifactContent) -> String {
    match content {
        ArtifactContent::String(s) => s.clone(),
        ArtifactContent::Bytes(b) => String::from_utf8_lossy(b).to_string(),
    }
}

#[test]
fn source_map_enabled() {
    let mut config = JsConfig::esm_deno();
    config.source_map = true;
    let output = JsCompiler
        .compile(&input(Term::constant(42)), &config)
        .unwrap();
    assert_eq!(output.artifacts().len(), 2);

    let js = artifact_to_string(&output.artifacts()[0].content);
    assert!(js.contains("//# sourceMappingURL=main.js.map"), "{js}");

    let map = artifact_to_string(&output.artifacts()[1].content);
    let parsed: serde_json::Value = serde_json::from_str(&map).unwrap();
    assert_eq!(parsed["version"], 3);
    assert!(!parsed["sources"].as_array().unwrap().is_empty());
    assert!(!parsed["mappings"].as_str().unwrap().is_empty());
    assert_eq!(parsed["sourcesContent"].as_array().unwrap().len(), 1);
}

#[test]
fn source_map_disabled() {
    let config = JsConfig::esm_deno();
    let output = JsCompiler
        .compile(&input(Term::constant(42)), &config)
        .unwrap();
    assert_eq!(output.artifacts().len(), 1);
    let js = artifact_to_string(&output.artifacts()[0].content);
    assert!(!js.contains("//# sourceMappingURL"), "{js}");
}

#[test]
fn safe_foreign_descriptor_validates_and_inlines() {
    let desc = json!({
        "module": "math",
        "name": "add",
        "arg_types": ["number", "number"],
        "return_type": "number"
    });
    let fd = validate_foreign_descriptor(&desc).unwrap();
    assert!(is_safe_for_inlining(&fd));

    let term = Term::Op {
        name: "add".to_string(),
        args: vec![
            Term::constant(Value::Number(Number::from(1))),
            Term::constant(Value::Number(Number::from(2))),
        ],
        source: ArrSource::Foreign("math".to_string()),
        lit: Some(desc),
    };
    let js = compile_to_string(&input(term), &JsConfig::esm_deno()).unwrap();
    assert!(js.contains(r#"globalThis["math"]"#), "{js}");
    assert!(js.contains(r#"["add"]"#), "{js}");
    assert!(!js.contains("__handl_rt.jsCall"), "{js}");
}

#[test]
fn unsafe_foreign_descriptor_rejected_by_validation() {
    let desc = json!({
        "module": "x",
        "name": "eval",
        "arg_types": [],
        "return_type": "void"
    });
    assert!(validate_foreign_descriptor(&desc).is_err());
}

#[test]
fn no_inline_foreign_emits_runtime_js_call() {
    let desc = json!({
        "module": "math",
        "name": "add",
        "arg_types": ["number", "number"],
        "return_type": "number"
    });
    let mut config = JsConfig::esm_deno();
    config.no_inline_foreign = true;
    let term = Term::Op {
        name: "add".to_string(),
        args: vec![
            Term::constant(Value::Number(Number::from(1))),
            Term::constant(Value::Number(Number::from(2))),
        ],
        source: ArrSource::Foreign("math".to_string()),
        lit: Some(desc),
    };
    let js = compile_to_string(&input(term), &config).unwrap();
    assert!(js.contains("__handl_rt.jsCall"), "{js}");
}

#[test]
fn generated_js_has_no_dangerous_call_patterns() {
    let js = compile_to_string(
        &input(Term::constant(Value::Number(Number::from(42)))),
        &JsConfig::esm_deno(),
    )
    .unwrap();
    assert!(!js.contains("eval("), "{js}");
    assert!(!js.contains("new Function("), "{js}");
    assert!(!js.contains("Function("), "{js}");
}

#[test]
fn dangerous_foreign_op_rejected_at_compile_time() {
    let desc = json!({
        "module": "x",
        "name": "eval",
        "arg_types": [],
        "return_type": "void"
    });
    let term = Term::Op {
        name: "eval".to_string(),
        args: vec![],
        source: ArrSource::Foreign("x".to_string()),
        lit: Some(desc),
    };
    assert!(compile_to_string(&input(term), &JsConfig::esm_deno()).is_err());
}

#[test]
fn fuzz_random_terms_compile() {
    let mut rng = rand::rng();
    for _ in 0..50 {
        let term = random_term(&mut rng, 3);
        match compile_to_string(&input(term), &JsConfig::esm_deno()) {
            Ok(js) => {
                assert!(!js.is_empty());
                assert!(!js.contains("eval("), "{js}");
                assert!(!js.contains("new Function("), "{js}");
                assert!(!js.contains("Function("), "{js}");
            }
            Err(_) => {}
        }
    }
}

#[test]
fn js_output_stream_writes_source() {
    let output = JsOutput::new("export default 42;");
    let mut buf = Vec::new();
    output.stream(&mut buf).unwrap();
    assert_eq!(String::from_utf8(buf).unwrap(), "export default 42;");
}

#[test]
fn js_forbid_unsafe_code() {
    let js = compile_to_string(
        &input(Term::constant(Value::Number(Number::from(42)))),
        &JsConfig::esm_deno(),
    )
    .unwrap();
    assert!(!js.contains("eval("), "{js}");
    assert!(!js.contains("new Function("), "{js}");
    assert!(!js.contains("Function("), "{js}");
}

#[test]
fn js_value_bounds_check() {
    let js = compile_to_string(
        &input(Term::loop_feedback(Term::constant(Value::Number(
            Number::from(42),
        )))),
        &JsConfig::esm_deno(),
    )
    .unwrap();
    assert!(js.contains("__handl_rt.isLeft"), "{js}");
    assert!(js.contains("__handl_rt.isRight"), "{js}");
    assert!(js.contains(r#"__either__ === "left""#), "{js}");
    assert!(js.contains(r#"__either__ === "right""#), "{js}");
}

fn random_term<R: Rng>(rng: &mut R, depth: usize) -> Term {
    if depth == 0 {
        let v = match rng.random_range(0..4) {
            0 => Value::Null,
            1 => Value::Bool(rng.random()),
            2 => Value::Number(Number::from(rng.random_range(-100..100))),
            3 => Value::String(format!("s{}", rng.random::<u32>())),
            _ => Value::Null,
        };
        return Term::constant(v);
    }
    match rng.random_range(0..5) {
        0 | 1 => random_term(rng, depth - 1),
        2 => {
            let a = random_term(rng, depth - 1);
            let b = random_term(rng, depth - 1);
            Term::seq(a, b)
        }
        3 => {
            let a = Term::constant(Value::Number(Number::from(rng.random_range(-100..100))));
            let b = Term::constant(Value::Number(Number::from(rng.random_range(-100..100))));
            Term::op_native("add", vec![a, b])
        }
        4 => Term::op_native_lit(
            "proj_env",
            vec![],
            Value::String(format!("f{}", rng.random::<u32>())),
        ),
        _ => random_term(rng, depth - 1),
    }
}
