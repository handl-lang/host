use handl_platform::Module;
use handl_platform::Function;

use super::hash::{Md5, Sha256};

/// The `crypto` module — hashing functions.
///
/// Provides functions callable as `crypto.sha256(data)` and
/// `crypto.md5(data)` in handl expressions.
///
/// Hashing functions accept a string (UTF-8 bytes) or an array of
/// integers (0–255) and return a lowercase hex-encoded digest.
#[derive(Clone, Copy)]
pub struct CryptoModule;

impl Module for CryptoModule {
    fn name(&self) -> &str {
        "crypto"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "sha256" => Some(&Sha256),
            "md5" => Some(&Md5),
            _ => None,
        }
    }
}
