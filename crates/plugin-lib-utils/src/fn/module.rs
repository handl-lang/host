use handl_platform::Function;
use handl_platform::Module;

use crate::r#fn::combinators::{
    Apply, Choice, Compose, Constantly, Converge, Fanout, Filter, Flip, Flow, Map, Match,
    Partial, Pipe, Reduce, Tap, Unless, When,
};
use crate::r#fn::higher_order::{CountBy, Every, Find, FlatMap, GroupBy, Partition, SomeFn};

/// The `fn` module — function combinator utilities.
///
/// Provides functions callable as `fn.pipe(v, f, g)`, `fn.compose(f, g)`,
/// `fn.map(f, xs)`, `fn.filter(f, xs)`, `fn.reduce(f, xs, init)`, etc. in handl expressions.
#[derive(Clone, Copy)]
pub struct FnModule;

impl Module for FnModule {
    fn name(&self) -> &str {
        "fn"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "pipe" => Some(&Pipe),
            "flow" => Some(&Flow),
            "compose" => Some(&Compose),
            "fanout" => Some(&Fanout),
            "choice" => Some(&Choice),
            "apply" => Some(&Apply),
            "map" => Some(&Map),
            "filter" => Some(&Filter),
            "reduce" => Some(&Reduce),
            "flat_map" => Some(&FlatMap),
            "find" => Some(&Find),
            "some" => Some(&SomeFn),
            "every" => Some(&Every),
            "group_by" => Some(&GroupBy),
            "partition" => Some(&Partition),
            "count_by" => Some(&CountBy),
            "tap" => Some(&Tap),
            "match" => Some(&Match),
            "constantly" => Some(&Constantly),
            "flip" => Some(&Flip),
            "partial" => Some(&Partial),
            "when" => Some(&When),
            "unless" => Some(&Unless),
            "converge" => Some(&Converge),
            _ => None,
        }
    }
}
