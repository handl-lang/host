mod sig;

pub use sig::{register_sig_ops, Sig, SigModule, SIG_OPS};

pub mod sig_stub;

#[cfg(test)]
mod sig_tests;
