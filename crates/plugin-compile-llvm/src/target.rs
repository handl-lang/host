//! LLVM target triple implementations.

use crate::config::{AArch64, LlvmTarget, X86_64};

impl LlvmTarget for X86_64 {
    fn name(&self) -> &str {
        "x86_64"
    }

    fn triple(&self) -> &str {
        x86_64_triple()
    }
}

impl LlvmTarget for AArch64 {
    fn name(&self) -> &str {
        "aarch64"
    }

    fn triple(&self) -> &str {
        aarch64_triple()
    }
}

/// The canonical x86-64 target triple.
pub fn x86_64_triple() -> &'static str {
    "x86_64-unknown-unknown"
}

/// The canonical AArch64 target triple.
pub fn aarch64_triple() -> &'static str {
    "aarch64-unknown-unknown"
}
