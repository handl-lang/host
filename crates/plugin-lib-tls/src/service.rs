use serde_json::Value;

use super::error::TlsError;

/// TLS operations implemented by a concrete backend.
///
/// `handl-lib-tls` ships with `StubTls` for 100% unit-test coverage. The
/// `handl-lib-tls-native` crate provides a real implementation backed by
/// `rcgen`, `rustls`, and `tokio-rustls`.
pub trait TlsService: Send + Sync {
    /// Returns TLS library/version information.
    fn version(&self) -> Value;

    /// Generates a self-signed certificate for the given hosts.
    ///
    /// Returns a JSON object with `cert`, `key`, and `fingerprint` strings.
    fn generate_self_signed(&self, hosts: &[String]) -> Result<Value, TlsError>;

    /// Parses a PEM certificate string and returns `{ count, valid }`.
    fn parse_cert(&self, pem: &str) -> Result<Value, TlsError>;

    /// Returns the SHA-256 hex fingerprint of the first certificate in `pem`.
    fn fingerprint(&self, pem: &str) -> Result<String, TlsError>;

    /// Builds a client configuration and returns a `config_id`.
    fn client_config(&self, roots: &[String], cert: Option<&str>, key: Option<&str>)
        -> Result<usize, TlsError>;

    /// Builds a server configuration and returns a `config_id`.
    fn server_config(&self, cert: &str, key: &str) -> Result<usize, TlsError>;

    /// Opens a TLS client connection to `host:port` using the client config
    /// identified by `config_id`. Returns a connection id.
    fn connect(&self, host: &str, port: u16, config_id: usize) -> Result<usize, TlsError>;

    /// Listens on `addr:port` using the server config identified by
    /// `config_id`. Returns a connection id for the first accepted TLS
    /// connection.
    fn accept(&self, addr: &str, port: u16, config_id: usize) -> Result<usize, TlsError>;

    /// Reads up to `len` bytes from `conn_id` as a UTF-8 string.
    fn read(&self, conn_id: usize, len: usize) -> Result<String, TlsError>;

    /// Writes `data` to `conn_id`.
    fn write(&self, conn_id: usize, data: &str) -> Result<(), TlsError>;

    /// Closes a connection.
    fn close_conn(&self, conn_id: usize) -> Result<(), TlsError>;

    /// Closes a listener.
    fn close_listener(&self, listener_id: usize) -> Result<(), TlsError>;
}
