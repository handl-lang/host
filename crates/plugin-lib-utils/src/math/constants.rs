use handl_platform::{EvalContext, EvalError, Function};

use super::helpers::num;

#[derive(Clone, Copy)]
pub struct Pi;
impl Function for Pi {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(num(std::f64::consts::PI))
    }
}

#[derive(Clone, Copy)]
pub struct E;
impl Function for E {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(num(std::f64::consts::E))
    }
}

#[derive(Clone, Copy)]
pub struct Nan;
impl Function for Nan {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(num(f64::NAN))
    }
}

#[derive(Clone, Copy)]
pub struct Infinity;
impl Function for Infinity {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(num(f64::INFINITY))
    }
}

#[derive(Clone, Copy)]
pub struct Tau;
impl Function for Tau {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(num(std::f64::consts::TAU))
    }
}
