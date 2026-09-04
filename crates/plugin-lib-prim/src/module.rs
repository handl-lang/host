use handl_platform::{Function, Module};

use super::ops::{Add, Alloc, Compare, Or};

static ADD: Add = Add;
static COMPARE: Compare = Compare;
static ALLOC: Alloc = Alloc;
static OR: Or = Or;

/// The `prim` module — the minimal FFI layer for handl's arrow-based stdlib.
///
/// Provides the four irreducible atom functions: `add`, `compare`, `alloc`,
/// and `or`. Everything else in the stdlib is built atop these via handl arrow
/// graphs.
#[derive(Clone, Copy)]
pub struct PrimModule;

impl Module for PrimModule {
    fn name(&self) -> &str {
        "prim"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "add" => Some(&ADD),
            "compare" => Some(&COMPARE),
            "alloc" => Some(&ALLOC),
            "or" => Some(&OR),
            _ => None,
        }
    }
}
