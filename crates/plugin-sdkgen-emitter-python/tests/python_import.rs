//! End-to-end: emit ctypes bindings and import them with a real Python
//! interpreter, exercising a live libc call.

use handl_plugin_sdkgen::{Arg, Emitter, EnumConst, Field, Function, Record, SdkgenModule};
use handl_plugin_sdkgen_emitter_python::PythonEmitter;

fn epoll_module() -> SdkgenModule {
    SdkgenModule {
        client_package: "ffi".into(),
        backend_id: "c".into(),
        target: "x86_64-unknown-linux-gnu".into(),
        library: "libc.so.6".into(),
        module: "epoll".into(),
        records: Vec::new(),
        functions: vec![
            Function {
                name: "epoll_create1".into(),
                port_name: "EpollCreate1".into(),
                port_args: vec![Arg {
                    name: "flags".into(),
                    port: "Int".into(),
                }],
                arg_descriptors: vec!["i32".into()],
                return_port: "Result<Int, Errno>".into(),
                ret_descriptor: "i32".into(),
                out_struct: None,
                variadic: false,
                errno_guard: true,
                c_decl: "int epoll_create1(int flags);".into(),
                doc: None,
            },
            Function {
                name: "epoll_ctl".into(),
                port_name: "EpollCtl".into(),
                port_args: vec![
                    Arg {
                        name: "epfd".into(),
                        port: "Int".into(),
                    },
                    Arg {
                        name: "operation".into(),
                        port: "Int".into(),
                    },
                    Arg {
                        name: "fd".into(),
                        port: "Int".into(),
                    },
                    Arg {
                        name: "event".into(),
                        port: "EpollCtlMsg".into(),
                    },
                ],
                arg_descriptors: vec![
                    "i32".into(),
                    "i32".into(),
                    "i32".into(),
                    "in_struct:{events:u32@0,data:u64@4}".into(),
                ],
                return_port: "Int".into(),
                ret_descriptor: "i32".into(),
                out_struct: None,
                variadic: false,
                errno_guard: true,
                c_decl: "int epoll_ctl(int epfd, int op, int fd, struct epoll_event *e);".into(),
                doc: None,
            },
        ],
        typedefs: Vec::new(),
        enum_constants: vec![
            EnumConst {
                name: "EPOLLIN".into(),
                value: 1,
                doc: None,
            },
            EnumConst {
                name: "EPOLL_CLOEXEC".into(),
                value: 524288,
                doc: None,
            },
            EnumConst {
                name: "EPOLL_CTL_ADD".into(),
                value: 1,
                doc: None,
            },
        ],
        imports: Vec::new(),
        source: None,
        per_record_files: false,
        diagnostics: Vec::new(),
    }
}

/// A record with a packed layout, to check the pad/offset machinery
/// against a real interpreter.
fn record_module() -> SdkgenModule {
    let mut m = epoll_module();
    m.records = vec![Record {
        name: "Packed".into(),
        fields: vec![
            Field {
                name: "a".into(),
                port: "Nat".into(),
                descriptor: "u8".into(),
                offset: 0,
                size: 1,
                ptr_depth: 0,
                is_const: false,
                doc: None,
            },
            Field {
                name: "b".into(),
                port: "Nat".into(),
                descriptor: "u64".into(),
                offset: 8,
                size: 8,
                ptr_depth: 0,
                is_const: false,
                doc: None,
            },
        ],
        size: 24,
        align: 8,
        doc: None,
    }];
    m
}

fn python3() -> Option<&'static str> {
    let ok = std::process::Command::new("python3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    ok.then_some("python3")
}

fn run_python(dir: &std::path::Path, script: &str) -> String {
    let py = python3().expect("python3");
    let out = std::process::Command::new(py)
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .output()
        .expect("run python3");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "python failed\nstdout: {stdout}\nstderr: {stderr}"
    );
    stdout
}

#[test]
fn generated_module_imports_and_calls_libc() {
    if python3().is_none() {
        eprintln!("python3 not available — skipping");
        return;
    }
    let dir = std::env::temp_dir().join("sdkgen-py-e2e");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    PythonEmitter::new().emit(&epoll_module(), &dir).unwrap();

    // Import runs the layout asserts; the body then makes real libc calls.
    let stdout = run_python(
        &dir,
        r#"
import ctypes, epoll
assert epoll.EPOLLIN == 1
assert epoll.EpollCtlMsg.data.offset == 4
libc = ctypes.CDLL("libc.so.6", use_errno=True)
pipefds = (ctypes.c_int * 2)()
assert libc.pipe(pipefds) == 0
r, w = pipefds[0], pipefds[1]
ok, epfd = epoll.epoll_create1(epoll.EPOLL_CLOEXEC)
assert ok, epfd
ev = epoll.EpollCtlMsg()
ev.events = epoll.EPOLLIN
ev.data = 7
assert epoll.epoll_ctl(epfd, epoll.EPOLL_CTL_ADD, r, ctypes.byref(ev)) == (True, 0)
# a bad fd must surface the errno tuple, not raise
ok2, err2 = epoll.epoll_ctl(epfd, epoll.EPOLL_CTL_ADD, -1, ctypes.byref(ev))
assert ok2 is False and err2 == 9, (ok2, err2)
print("ok")
"#,
    );
    assert!(stdout.contains("ok"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn generated_record_layout_matches_python() {
    if python3().is_none() {
        eprintln!("python3 not available — skipping");
        return;
    }
    let dir = std::env::temp_dir().join("sdkgen-py-record");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    PythonEmitter::new().emit(&record_module(), &dir).unwrap();

    let stdout = run_python(
        &dir,
        r#"
import ctypes, epoll
assert ctypes.sizeof(epoll.Packed) == 24
assert epoll.Packed.a.offset == 0
assert epoll.Packed.b.offset == 8
print("ok")
"#,
    );
    assert!(stdout.contains("ok"));
    let _ = std::fs::remove_dir_all(&dir);
}
