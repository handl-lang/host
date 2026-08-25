//! Library loading and config tests for the tooling host.

use handl_plugin_diag_expression::ExpressionDiagnostics;
use handl_plugin_fmt_expression::ExpressionFormatter;
use handl_tooling::{Library, ToolingHost};
use handl_vm_common::{Module, ModuleHost};
use handl_vm_values_json::{Context, Error, Function, NonFunctionValue};

static GREET: Greet = Greet;

#[derive(Clone, Copy)]
struct HelloModule;

impl Module for HelloModule {
    fn name(&self) -> &str {
        "hello"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "greet" => Some(&GREET),
            _ => None,
        }
    }
}

struct Greet;

impl Function<NonFunctionValue> for Greet {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let name = args.first().and_then(|v| v.as_str()).unwrap_or("world");
        Ok(NonFunctionValue::new(format!("Hello, {name}!")))
    }
}

#[derive(Clone, Copy)]
struct HelloLib;

impl Library for HelloLib {
    fn name(&self) -> &str {
        "hello"
    }

    fn install(&self, host: ModuleHost) -> ModuleHost {
        host.with(HelloModule)
    }
}

fn host() -> ToolingHost {
    let mut host = ToolingHost::new(
        handl_kit_canonical::CanonicalKit::new(),
        Box::new(ExpressionFormatter::new()),
        Box::new(ExpressionDiagnostics::new()),
    );
    host.register_lib("hello", || Box::new(HelloLib));
    host.load_lib_by_name("hello").unwrap();
    host
}

#[test]
fn tooling_loads_lib_and_evals() {
    let host = host();
    let output = host
        .kit()
        .parse_module_recovered("hello.greet(\"world\")");
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let values = host.kit().eval(&output.stmts).unwrap().values;
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].as_str(), Some("Hello, world!"));
}

#[test]
fn tooling_unknown_lib_errors() {
    let mut host = ToolingHost::new(
        handl_kit_canonical::CanonicalKit::new(),
        Box::new(ExpressionFormatter::new()),
        Box::new(ExpressionDiagnostics::new()),
    );
    let err = host.load_lib_by_name("missing").unwrap_err();
    assert!(format!("{err}").contains("missing"));
}

#[test]
fn tooling_config_loads_libraries() {
    let dir = std::env::temp_dir().join(format!("handl_config_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("handl.config.json");
    std::fs::write(&path, r#"{"lib": ["hello"]}"#).unwrap();

    let mut host = ToolingHost::new(
        handl_kit_canonical::CanonicalKit::new(),
        Box::new(ExpressionFormatter::new()),
        Box::new(ExpressionDiagnostics::new()),
    );
    host.register_lib("hello", || Box::new(HelloLib));
    host.load_config(&path).unwrap();

    let output = host.kit().parse_module_recovered("hello.greet(\"config\")");
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let values = host.kit().eval(&output.stmts).unwrap().values;
    assert_eq!(values[0].as_str(), Some("Hello, config!"));

    std::fs::remove_dir_all(&dir).ok();
}
