use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::error::TlsError;
use super::service::TlsService;

struct StubInner {
    next_id: AtomicUsize,
    buffers: Mutex<HashMap<usize, Vec<u8>>>,
}

/// A test-only `TlsService` that never performs real cryptography or I/O.
///
/// It returns deterministic, well-formed values for the certificate and
/// configuration functions and maintains in-memory buffers for the
/// connection-oriented functions.
#[derive(Clone)]
pub struct StubTls {
    inner: Arc<StubInner>,
}

impl StubTls {
    /// Creates a new `StubTls`.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(StubInner {
                next_id: AtomicUsize::new(1),
                buffers: Mutex::new(HashMap::new()),
            }),
        }
    }

    fn next_id(&self) -> usize {
        self.inner.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

impl TlsService for StubTls {
    fn version(&self) -> Value {
        json!({
            "library": "stub",
            "versions": ["1.2", "1.3"]
        })
    }

    fn generate_self_signed(&self, hosts: &[String]) -> Result<Value, TlsError> {
        if hosts.is_empty() {
            return Err(TlsError::InvalidInput {
                message: "at least one host is required".to_string(),
            });
        }
        Ok(json!({
            "cert": "-----BEGIN CERTIFICATE-----\nSTUB\n-----END CERTIFICATE-----\n",
            "key": "-----BEGIN PRIVATE KEY-----\nSTUB\n-----END PRIVATE KEY-----\n",
            "fingerprint": "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
        }))
    }

    fn parse_cert(&self, pem: &str) -> Result<Value, TlsError> {
        if pem.is_empty() {
            return Ok(json!({"count": 0, "valid": false}));
        }
        if pem == "invalid" {
            return Err(TlsError::InvalidPem {
                message: "stub invalid PEM".to_string(),
            });
        }
        Ok(json!({"count": 1, "valid": true}))
    }

    fn fingerprint(&self, pem: &str) -> Result<String, TlsError> {
        if pem.is_empty() || pem == "invalid" {
            return Err(TlsError::Fingerprint {
                message: "no certificate found".to_string(),
            });
        }
        Ok("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".to_string())
    }

    fn client_config(
        &self,
        roots: &[String],
        _cert: Option<&str>,
        _key: Option<&str>,
    ) -> Result<usize, TlsError> {
        if roots.is_empty() {
            return Err(TlsError::InvalidConfig {
                message: "at least one root is required".to_string(),
            });
        }
        Ok(self.next_id())
    }

    fn server_config(&self, cert: &str, _key: &str) -> Result<usize, TlsError> {
        if cert.is_empty() {
            return Err(TlsError::InvalidConfig {
                message: "cert is required".to_string(),
            });
        }
        Ok(self.next_id())
    }

    fn connect(&self, host: &str, _port: u16, _config_id: usize) -> Result<usize, TlsError> {
        if host == "error" {
            return Err(TlsError::Io {
                message: "stub connect error".to_string(),
            });
        }
        Ok(self.next_id())
    }

    fn accept(&self, addr: &str, _port: u16, _config_id: usize) -> Result<usize, TlsError> {
        if addr == "error" {
            return Err(TlsError::Io {
                message: "stub accept error".to_string(),
            });
        }
        Ok(self.next_id())
    }

    fn read(&self, conn_id: usize, len: usize) -> Result<String, TlsError> {
        if conn_id == 0 {
            return Err(TlsError::Io {
                message: "stub read error".to_string(),
            });
        }
        let mut buffers = self.inner.buffers.lock().unwrap();
        let mut taken = Vec::new();
        if let Some(data) = buffers.get_mut(&conn_id) {
            let take = len.min(data.len());
            taken = data.drain(..take).collect();
        }
        Ok(String::from_utf8_lossy(&taken).to_string())
    }

    fn write(&self, conn_id: usize, data: &str) -> Result<(), TlsError> {
        if conn_id == 0 {
            return Err(TlsError::Io {
                message: "stub write error".to_string(),
            });
        }
        let mut buffers = self.inner.buffers.lock().unwrap();
        buffers
            .entry(conn_id)
            .or_default()
            .extend_from_slice(data.as_bytes());
        Ok(())
    }

    fn close_conn(&self, conn_id: usize) -> Result<(), TlsError> {
        if conn_id == 0 {
            return Err(TlsError::Io {
                message: "stub close_conn error".to_string(),
            });
        }
        let mut buffers = self.inner.buffers.lock().unwrap();
        buffers.remove(&conn_id);
        Ok(())
    }

    fn close_listener(&self, listener_id: usize) -> Result<(), TlsError> {
        if listener_id == 0 {
            return Err(TlsError::Io {
                message: "stub close_listener error".to_string(),
            });
        }
        Ok(())
    }
}
