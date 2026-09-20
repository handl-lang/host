use crate::config::SdkgenConfig;
use handl_plugin_sdkgen::{Arg, Function};

fn func(name: &str, arg_names: &[&str]) -> Function {
    Function {
        name: name.into(),
        port_name: "F".into(),
        port_args: arg_names
            .iter()
            .map(|n| Arg {
                name: n.to_string(),
                port: "Int".into(),
            })
            .collect(),
        arg_descriptors: arg_names.iter().map(|_| "i32".into()).collect(),
        return_port: "Int".into(),
        ret_descriptor: "i32".into(),
        out_struct: None,
        variadic: false,
        errno_guard: true,
        c_decl: String::new(),
        doc: None,
    }
}

fn cfg(yaml: &str) -> SdkgenConfig {
    serde_yaml::from_str(yaml).unwrap()
}

#[test]
fn should_skip() {
    let c = cfg("symbols:\n  old_fn:\n    skip: true\n  keep_fn:\n    skip: false\n");
    assert!(c.should_skip("old_fn"));
    assert!(!c.should_skip("keep_fn"));
    assert!(!c.should_skip("unlisted"));
}

#[test]
fn override_out_buf_creates_no_port_arg() {
    // recv(fd, buf, n, flags) -> override ["i32","out_buf:64","i32"]:
    // out_buf expands to (ptr,len) at the C call but consumes no handl arg.
    let c = cfg("symbols:\n  recv:\n    args: [i32, 'out_buf:64', i32]\n");
    let mut f = func("recv", &["fd", "buf", "n", "flags"]);
    c.apply_to_function("recv", &mut f);
    assert_eq!(f.arg_descriptors, &["i32", "out_buf:64", "i32"]);
    // Port args are named by C-argument index: i32→carg0 (fd),
    // out_buf→cargs1-2, i32→carg3 (flags — not "n" by descriptor index).
    assert_eq!(f.port_args.len(), 2);
    assert_eq!(f.port_args[0].name, "fd");
    assert_eq!(f.port_args[1].name, "flags");
}

#[test]
fn override_in_buf_consumes_one_port_arg() {
    let c = cfg("symbols:\n  send:\n    args: [i32, in_buf, i32]\n");
    let mut f = func("send", &["fd", "buf", "len", "flags"]);
    c.apply_to_function("send", &mut f);
    assert_eq!(f.port_args.len(), 3);
    assert_eq!(f.port_args[1].port, "Bytes");
    assert_eq!(f.port_args[2].name, "flags");
}

#[test]
fn override_ret_and_descriptor() {
    let c = cfg("symbols:\n  open_:\n    ret: Handle\n");
    let mut f = func("open_", &[]);
    c.apply_to_function("open_", &mut f);
    assert_eq!(f.return_port, "Handle");
    assert_eq!(f.ret_descriptor, "handle");

    let c = cfg("symbols:\n  raw:\n    descriptor: cstr\n");
    let mut f = func("raw", &[]);
    c.apply_to_function("raw", &mut f);
    assert_eq!(f.ret_descriptor, "cstr");
}

#[test]
fn rescue_requires_args() {
    let c = cfg("symbols:\n  noargs:\n    ret: Int\n");
    assert!(c.rescue_function("noargs").is_none());
    assert!(c.rescue_function("unlisted").is_none());
}

#[test]
fn rescue_builds_function_from_override() {
    let c = cfg("symbols:\n  weird:\n    args: [cstr, 'out_buf:128']\n    ret: String\n");
    let f = c.rescue_function("weird").expect("rescued");
    assert_eq!(f.name, "weird");
    assert_eq!(f.port_name, "Weird");
    assert_eq!(f.arg_descriptors, &["cstr", "out_buf:128"]);
    assert_eq!(f.port_args.len(), 1); // only cstr consumes
    assert_eq!(f.port_args[0].name, "__arg_0");
    // out_buf forces the visible return to Bytes — the emitter unwraps
    // { ret, buf } → buf regardless of the `ret` override.
    assert_eq!(f.return_port, "Bytes");
    assert_eq!(f.ret_descriptor, "cstr");
}
