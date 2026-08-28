use std::thread;
use std::time::Duration;

use handl_lib_tls::TlsService;

use super::NativeTls;

#[test]
fn generate_self_signed_round_trip() {
    let tls = NativeTls::new();
    let value = tls.generate_self_signed(&["localhost".to_string()]).unwrap();
    let cert = value["cert"].as_str().unwrap().to_string();
    let fp = tls.fingerprint(&cert).unwrap();
    assert_eq!(fp.len(), 64);
    assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));

    let parsed = tls.parse_cert(&cert).unwrap();
    assert_eq!(parsed["count"], 1);
    assert_eq!(parsed["valid"], true);
}

#[test]
fn generate_self_signed_empty_hosts_errors() {
    let tls = NativeTls::new();
    let result = tls.generate_self_signed(&[]);
    assert!(result.is_err());
}

#[test]
fn parse_cert_empty_pem() {
    let tls = NativeTls::new();
    let result = tls.parse_cert("").unwrap();
    assert_eq!(result["count"], 0);
    assert_eq!(result["valid"], false);
}

#[test]

#[ignore = "rustls crypto provider not configured"]
fn client_and_server_config() {
    let tls = NativeTls::new();
    let value = tls
        .generate_self_signed(&["localhost".to_string()])
        .unwrap();
    let cert = value["cert"].as_str().unwrap();
    let key = value["key"].as_str().unwrap();

    let client_id = tls.client_config(&[cert.to_string()], None, None).unwrap();
    let server_id = tls.server_config(cert, key).unwrap();
    assert!(client_id > 0);
    assert!(server_id > 0);
}

#[test]

#[ignore = "rustls crypto provider not configured"]
fn connect_accept_echo() {
    let tls = NativeTls::new();
    let value = tls
        .generate_self_signed(&["localhost".to_string()])
        .unwrap();
    let cert = value["cert"].as_str().unwrap();
    let key = value["key"].as_str().unwrap();

    let client_config = tls.client_config(&[cert.to_string()], None, None).unwrap();
    let server_config = tls.server_config(cert, key).unwrap();

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let tls2 = tls.clone();
    let server = thread::spawn(move || tls2.accept("127.0.0.1", port, server_config));

    thread::sleep(Duration::from_millis(100));

    let mut client_id = None;
    for _ in 0..20 {
        match tls.connect("localhost", port, client_config) {
            Ok(id) => {
                client_id = Some(id);
                break;
            }
            Err(e) => {
                eprintln!("connect error: {e}");
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
    let client_id = client_id.expect("failed to connect");

    let server_id = server.join().unwrap().unwrap();

    // Client writes, server reads it back.
    tls.write(client_id, "hello").unwrap();
    let read = tls.read(server_id, 5).unwrap();
    assert_eq!(read, "hello");

    // Server writes back, client reads it.
    tls.write(server_id, "world").unwrap();
    let read = tls.read(client_id, 5).unwrap();
    assert_eq!(read, "world");

    tls.close_conn(client_id).unwrap();
    tls.close_conn(server_id).unwrap();
}
