//! # handl-lib-tls-native
//!
//! Concrete `tokio-rustls` implementation of `handl_plugin_lib_tls::TlsService`.

use std::collections::HashMap;
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use handl_plugin_lib_tls::{TlsError, TlsService};
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig as RustlsClientConfig, RootCertStore, ServerConfig as RustlsServerConfig};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio::sync::Mutex as TokioMutex;
use tokio_rustls::TlsConnector;
use tokio_rustls::TlsAcceptor;

struct NativeInner {
    next_id: AtomicUsize,
    client_configs: Mutex<HashMap<usize, Arc<RustlsClientConfig>>>,
    server_configs: Mutex<HashMap<usize, Arc<RustlsServerConfig>>>,
    connections: TokioMutex<HashMap<usize, Conn>>,
    runtime: Runtime,
}

enum Conn {
    Client(tokio_rustls::client::TlsStream<TcpStream>),
    Server(tokio_rustls::server::TlsStream<TcpStream>),
}

/// A `TlsService` backed by `rcgen` for certificates and `tokio-rustls`
/// for network I/O.
#[derive(Clone)]
pub struct NativeTls {
    inner: Arc<NativeInner>,
}

impl Default for NativeTls {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeTls {
    /// Creates a new `NativeTls`.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(NativeInner {
                next_id: AtomicUsize::new(1),
                client_configs: Mutex::new(HashMap::new()),
                server_configs: Mutex::new(HashMap::new()),
                connections: TokioMutex::new(HashMap::new()),
                runtime: Runtime::new().expect("tokio runtime"),
            }),
        }
    }

    fn next_id(&self) -> usize {
        self.inner.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

impl TlsService for NativeTls {
    fn version(&self) -> Value {
        json!({
            "library": "tokio-rustls",
            "versions": ["1.2", "1.3"]
        })
    }

    fn generate_self_signed(&self, hosts: &[String]) -> Result<Value, TlsError> {
        if hosts.is_empty() {
            return Err(TlsError::InvalidInput {
                message: "at least one host is required".to_string(),
            });
        }

        let CertifiedKey { cert, key_pair } =
            generate_simple_self_signed(hosts.to_vec()).map_err(|e| TlsError::Generation {
                message: e.to_string(),
            })?;

        let cert_pem = cert.pem();
        let key_pem = key_pair.serialize_pem();
        let fp = fingerprint(&cert_pem)?;

        Ok(json!({
            "cert": cert_pem,
            "key": key_pem,
            "fingerprint": fp,
        }))
    }

    fn parse_cert(&self, pem: &str) -> Result<Value, TlsError> {
        let mut cursor = Cursor::new(pem.as_bytes());
        let certs: Vec<_> = rustls_pemfile::certs(&mut cursor)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| TlsError::InvalidPem {
                message: e.to_string(),
            })?;

        Ok(json!({
            "count": certs.len(),
            "valid": !certs.is_empty(),
        }))
    }

    fn fingerprint(&self, pem: &str) -> Result<String, TlsError> {
        fingerprint(pem)
    }

    fn client_config(
        &self,
        roots: &[String],
        cert: Option<&str>,
        key: Option<&str>,
    ) -> Result<usize, TlsError> {
        if roots.is_empty() {
            return Err(TlsError::InvalidConfig {
                message: "at least one root is required".to_string(),
            });
        }

        let mut root_store = RootCertStore::empty();
        for root in roots {
            add_roots(root, &mut root_store)?;
        }

        let config = match (cert, key) {
            (Some(cert), Some(key)) => {
                let cert_chain = parse_certs(cert)?;
                let key = parse_key(key)?;
                RustlsClientConfig::builder()
                    .with_root_certificates(Arc::new(root_store))
                    .with_client_auth_cert(cert_chain, key)
                    .map_err(|e| TlsError::InvalidConfig {
                        message: e.to_string(),
                    })?
            }
            _ => RustlsClientConfig::builder()
                .with_root_certificates(Arc::new(root_store))
                .with_no_client_auth(),
        };

        let id = self.next_id();
        self.inner
            .client_configs
            .lock()
            .unwrap()
            .insert(id, Arc::new(config));
        Ok(id)
    }

    fn server_config(&self, cert: &str, key: &str) -> Result<usize, TlsError> {
        if cert.is_empty() {
            return Err(TlsError::InvalidConfig {
                message: "cert is required".to_string(),
            });
        }

        let cert_chain = parse_certs(cert)?;
        let key = parse_key(key)?;

        let config = RustlsServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, key)
            .map_err(|e| TlsError::InvalidConfig {
                message: e.to_string(),
            })?;

        let id = self.next_id();
        self.inner
            .server_configs
            .lock()
            .unwrap()
            .insert(id, Arc::new(config));
        Ok(id)
    }

    fn connect(&self, host: &str, port: u16, config_id: usize) -> Result<usize, TlsError> {
        let client_config = self
            .inner
            .client_configs
            .lock()
            .unwrap()
            .get(&config_id)
            .cloned()
            .ok_or_else(|| TlsError::InvalidConfig {
                message: "client config not found".to_string(),
            })?;

        let inner = self.inner.clone();
        let host = host.to_string();
        self.inner.runtime.block_on(async move {
            let stream = TcpStream::connect((host.clone(), port))
                .await
                .map_err(|e| TlsError::Io { message: e.to_string() })?;
            let server_name = ServerName::try_from(host).map_err(|_| TlsError::InvalidInput {
                message: "invalid server name".to_string(),
            })?;
            let connector = TlsConnector::from(client_config);
            let tls = connector
                .connect(server_name, stream)
                .await
                .map_err(|e| TlsError::Io { message: e.to_string() })?;
            let id = inner.next_id.fetch_add(1, Ordering::Relaxed);
            inner.connections.lock().await.insert(id, Conn::Client(tls));
            Ok(id)
        })
    }

    fn accept(&self, addr: &str, port: u16, config_id: usize) -> Result<usize, TlsError> {
        let server_config = self
            .inner
            .server_configs
            .lock()
            .unwrap()
            .get(&config_id)
            .cloned()
            .ok_or_else(|| TlsError::InvalidConfig {
                message: "server config not found".to_string(),
            })?;

        let inner = self.inner.clone();
        let bind_addr: SocketAddr = format!("{}:{}", addr, port)
            .parse()
            .map_err(|e: std::net::AddrParseError| TlsError::InvalidInput {
                message: e.to_string(),
            })?;

        self.inner.runtime.block_on(async move {
            let listener = TcpListener::bind(bind_addr)
                .await
                .map_err(|e| TlsError::Io { message: e.to_string() })?;
            let (stream, _) = listener
                .accept()
                .await
                .map_err(|e| TlsError::Io { message: e.to_string() })?;
            let acceptor = TlsAcceptor::from(server_config);
            let tls = acceptor
                .accept(stream)
                .await
                .map_err(|e| TlsError::Io { message: e.to_string() })?;
            let id = inner.next_id.fetch_add(1, Ordering::Relaxed);
            inner.connections.lock().await.insert(id, Conn::Server(tls));
            Ok(id)
        })
    }

    fn read(&self, conn_id: usize, len: usize) -> Result<String, TlsError> {
        let inner = self.inner.clone();
        self.inner.runtime.block_on(async move {
            let mut conns = inner.connections.lock().await;
            let conn = conns.get_mut(&conn_id).ok_or(TlsError::Io {
                message: "connection not found".to_string(),
            })?;

            let mut buf = vec![0u8; len];
            let n = match conn {
                Conn::Client(s) => s.read(&mut buf).await,
                Conn::Server(s) => s.read(&mut buf).await,
            }
            .map_err(|e| TlsError::Io { message: e.to_string() })?;
            buf.truncate(n);
            String::from_utf8(buf).map_err(|e| TlsError::Io {
                message: format!("invalid utf-8: {e}"),
            })
        })
    }

    fn write(&self, conn_id: usize, data: &str) -> Result<(), TlsError> {
        let inner = self.inner.clone();
        let data = data.as_bytes().to_vec();
        self.inner.runtime.block_on(async move {
            let mut conns = inner.connections.lock().await;
            let conn = conns.get_mut(&conn_id).ok_or(TlsError::Io {
                message: "connection not found".to_string(),
            })?;

            match conn {
                Conn::Client(s) => s.write_all(&data).await,
                Conn::Server(s) => s.write_all(&data).await,
            }
            .map_err(|e| TlsError::Io { message: e.to_string() })
        })
    }

    fn close_conn(&self, conn_id: usize) -> Result<(), TlsError> {
        let inner = self.inner.clone();
        self.inner.runtime.block_on(async move {
            inner
                .connections
                .lock()
                .await
                .remove(&conn_id)
                .ok_or(TlsError::Io {
                    message: "connection not found".to_string(),
                })?;
            Ok(())
        })
    }

    fn close_listener(&self, _listener_id: usize) -> Result<(), TlsError> {
        Ok(())
    }
}

fn fingerprint(pem: &str) -> Result<String, TlsError> {
    let mut cursor = Cursor::new(pem.as_bytes());
    let mut certs = rustls_pemfile::certs(&mut cursor);

    let cert = certs.next().ok_or(TlsError::Fingerprint {
        message: "no certificate found".to_string(),
    })?;

    let cert = cert.map_err(|e| TlsError::Fingerprint {
        message: e.to_string(),
    })?;

    let hash = Sha256::digest(cert.as_ref());
    Ok(hex::encode(hash))
}

fn add_roots(pem: &str, store: &mut RootCertStore) -> Result<(), TlsError> {
    let mut cursor = Cursor::new(pem.as_bytes());
    for cert in rustls_pemfile::certs(&mut cursor) {
        let cert = cert.map_err(|e| TlsError::InvalidConfig {
            message: e.to_string(),
        })?;
        store.add(cert.into_owned()).map_err(|e| TlsError::InvalidConfig {
            message: e.to_string(),
        })?;
    }
    Ok(())
}

fn parse_certs(pem: &str) -> Result<Vec<CertificateDer<'static>>, TlsError> {
    let mut cursor = Cursor::new(pem.as_bytes());
    let certs: Vec<_> = rustls_pemfile::certs(&mut cursor)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| TlsError::InvalidConfig {
            message: e.to_string(),
        })?;
    Ok(certs.into_iter().map(|c| c.into_owned()).collect())
}

fn parse_key(pem: &str) -> Result<PrivateKeyDer<'static>, TlsError> {
    let mut cursor = Cursor::new(pem.as_bytes());
    let key = rustls_pemfile::private_key(&mut cursor)
        .map_err(|e| TlsError::InvalidConfig {
            message: e.to_string(),
        })?
        .ok_or(TlsError::InvalidConfig {
            message: "no private key found".to_string(),
        })?;
    Ok(key.clone_key())
}

#[cfg(test)]
mod tests;

pub mod plugin;
pub use plugin::LibraryPlugin;
