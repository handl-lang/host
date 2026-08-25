use serde_json::Value;

use crate::error::CompilerError;

/// A single pluggable compiler pass.
pub trait Pass: Send + Sync {
    /// Unique identifier for this pass.
    fn id(&self) -> &str;

    /// Run the pass against the input JSON value.
    fn run(&self, input: Value) -> Result<Value, CompilerError>;
}

/// A host that runs a sequence of passes in order, feeding each
/// output to the next.
pub struct PassHost {
    passes: Vec<Box<dyn Pass + 'static>>,
}

impl PassHost {
    /// Create an empty pass host.
    pub fn new() -> Self {
        Self { passes: Vec::new() }
    }

    /// Register a pass in this host.
    pub fn register(&mut self, pass: impl Pass + 'static) {
        self.passes.push(Box::new(pass));
    }

    /// Run all registered passes in order.
    pub fn run(&self, input: Value) -> Result<Value, CompilerError> {
        let mut output = input;
        for pass in &self.passes {
            output = pass.run(output)?;
        }
        Ok(output)
    }
}

impl Default for PassHost {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AppendPass(&'static str);

    impl Pass for AppendPass {
        fn id(&self) -> &str {
            self.0
        }

        fn run(&self, input: Value) -> Result<Value, CompilerError> {
            let mut obj = match input {
                Value::Object(m) => m,
                _ => serde_json::Map::new(),
            };
            obj.insert(self.0.to_string(), Value::Bool(true));
            Ok(Value::Object(obj))
        }
    }

    struct FailingPass;

    impl Pass for FailingPass {
        fn id(&self) -> &str {
            "fail"
        }

        fn run(&self, _input: Value) -> Result<Value, CompilerError> {
            Err(CompilerError::PassFailed("boom".to_string()))
        }
    }

    #[test]
    fn empty_host_returns_input_unchanged() {
        let host = PassHost::new();
        let input = serde_json::json!({"hello": true});
        assert_eq!(host.run(input.clone()).unwrap(), input);
    }

    #[test]
    fn passes_run_in_registration_order() {
        let mut host = PassHost::new();
        host.register(AppendPass("first"));
        host.register(AppendPass("second"));
        host.register(AppendPass("third"));

        let out = host.run(serde_json::Value::Null).unwrap();
        let obj = out.as_object().unwrap();
        let keys: Vec<_> = obj.keys().collect();
        assert_eq!(keys, vec!["first", "second", "third"]);
    }

    #[test]
    fn pass_failure_propagates() {
        let mut host = PassHost::new();
        host.register(AppendPass("first"));
        host.register(FailingPass);
        host.register(AppendPass("second"));

        let err = host.run(serde_json::Value::Null).unwrap_err();
        assert!(matches!(err, CompilerError::PassFailed(_)));
    }
}
