use handl_vm_values_json::NonFunctionValue;

use handl_vm::{Context, Function};
use handl_vm_values_json::Error;

fn require_one_arg(args: &[NonFunctionValue]) -> Result<&NonFunctionValue, Error> {
    args.first().ok_or(Error::ArgumentCount {
        expected: 1,
        got: 0,
    })
}

fn s(value: &str) -> NonFunctionValue {
    NonFunctionValue::new(value.to_string())
}

#[derive(Clone, Copy)]
pub struct TypeOf;
impl Function<NonFunctionValue> for TypeOf {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let arg = require_one_arg(args)?;
        let type_name = if arg.is_null() {
            "null"
        } else if arg.is_bool() {
            "bool"
        } else if arg.is_number() {
            "num"
        } else if arg.is_string() {
            "str"
        } else if arg.is_array() {
            "arr"
        } else if arg.is_object() {
            "obj"
        } else {
            "unknown"
        };
        Ok(s(type_name))
    }
}
