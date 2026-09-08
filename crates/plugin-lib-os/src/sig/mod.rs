mod sig;

pub use sig::{SIG_OPS, Sig, SigModule, register_sig_ops};

pub mod sig_stub;

#[cfg(test)]
mod sig_tests;
