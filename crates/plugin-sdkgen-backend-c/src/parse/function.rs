//! Function declarations → `Function`.

use clang_sys::*;
use handl_plugin_sdkgen::{Arg, Function};
use heck::ToUpperCamelCase;

use crate::cx::{cursor_name, doc_comment, type_spelling};
use crate::map::{map_arg_with_peek, map_return_type};
use crate::parse::ParseCtx;

pub(crate) fn parse_function(cursor: CXCursor, ctx: &ParseCtx) -> Result<Function, String> {
    let name = cursor_name(cursor);
    if name.is_empty() {
        return Err("unnamed function".into());
    }
    let doc = doc_comment(cursor);
    let port_name = name.to_upper_camel_case();
    let c_decl = type_spelling(unsafe { clang_getCursorType(cursor) });

    let n_args = unsafe { clang_Cursor_getNumArguments(cursor) };
    if n_args < 0 {
        return Err(format!("function {name} argument count unknown"));
    }
    let n = n_args as usize;
    let mut cursors = Vec::with_capacity(n);
    for i in 0..n {
        cursors.push(unsafe { clang_Cursor_getArgument(cursor, i as std::ffi::c_uint) });
    }

    let ret_type = unsafe { clang_getCursorResultType(cursor) };
    let variadic = unsafe { clang_isFunctionTypeVariadic(clang_getCursorType(cursor)) != 0 };

    let mut port_args = Vec::new();
    let mut arg_descriptors = Vec::new();
    let mut out_struct: Option<String> = None;

    let mut i = 0;
    while i < cursors.len() {
        let arg = cursors[i];
        let arg_name = {
            let n = cursor_name(arg);
            // Unnamed params (`SSL_CTX_free(SSL_CTX *)`) must still emit
            // a port name — an empty string would render as `()` and
            // silently drop the argument from the ffi.call.
            if n.is_empty() {
                format!("__arg_{i}")
            } else {
                n
            }
        };
        let arg_ty = unsafe { clang_getCursorType(arg) };
        if arg_ty.kind == CXType_Void {
            // `int f(void)` — the void placeholder is not a real argument.
            i += 1;
            continue;
        }
        let next = cursors.get(i + 1).copied();

        let info = map_arg_with_peek(arg_ty, next, ctx.pointer_width, variadic)?;
        if info.is_size_pair {
            // The pointer+length pair collapses to one handl argument
            // (Bytes) whose descriptor expands to two C args.
            if info.consumes {
                port_args.push(Arg {
                    name: arg_name,
                    port: info.port,
                });
            }
            arg_descriptors.push(info.descriptor);
            i += 2;
            continue;
        }

        if info.is_out_struct {
            match struct_descriptor("out_struct", &info.record_name, ctx) {
                Ok(record) => {
                    if out_struct.is_some() {
                        return Err(format!("function {name} has multiple out-struct params"));
                    }
                    arg_descriptors.push(record);
                    out_struct = info.record_name.clone();
                }
                Err(_) => {
                    // Unknown or incomplete record (opaque SSL*, FILE*, ...):
                    // it is a handle the callee dereferences, not a caller-
                    // allocated out-param — pass the pointer through.
                    port_args.push(Arg {
                        name: arg_name,
                        port: "Handle".into(),
                    });
                    arg_descriptors.push("ptr".into());
                }
            }
            i += 1;
            continue;
        }

        if info.is_in_struct {
            match struct_descriptor("in_struct", &info.record_name, ctx) {
                Ok(record) => {
                    // The port arg is typed as the record itself (timespec,
                    // sockaddr_in, ...) — not a per-function `*Msg` shape.
                    port_args.push(Arg {
                        name: arg_name,
                        port: info.port,
                    });
                    arg_descriptors.push(record);
                }
                Err(_) => {
                    // Opaque or unbuildable record — pass through.
                    port_args.push(Arg {
                        name: arg_name,
                        port: "Handle".into(),
                    });
                    arg_descriptors.push("ptr".into());
                }
            }
            i += 1;
            continue;
        }

        if info.consumes {
            port_args.push(Arg {
                name: arg_name,
                port: info.port,
            });
        }
        arg_descriptors.push(info.descriptor);
        i += 1;
    }

    let (ret_port, ret_descriptor) = map_return_type(ret_type, ctx.pointer_width)?;
    // An out-struct param turns the port return into `{ret, buf}` — `ret`
    // stays meaningful (status vs. payload), the emitter declares a
    // `{Port}Result` record rather than unwrapping and dropping it.
    let return_port = if out_struct.is_some() {
        format!("{port_name}Result")
    } else {
        ret_port
    };

    Ok(Function {
        name,
        port_name,
        port_args,
        arg_descriptors,
        return_port,
        ret_descriptor,
        out_struct,
        variadic,
        errno_guard: true,
        c_decl,
        doc,
    })
}

/// Render `{prefix}:{field:desc@off[:size],...}` from a previously parsed
/// record. Offsets and slot widths come from libclang's real target layout
/// (`clang_Cursor_getOffsetOfField`/`Type_getSizeOf`), so the marshaller
/// needs no layout assumptions of its own.
fn struct_descriptor(
    prefix: &str,
    name: &Option<String>,
    ctx: &ParseCtx,
) -> Result<String, String> {
    let name = name.as_ref().ok_or("missing struct name")?;
    let rec = ctx
        .records
        .borrow()
        .get(name)
        .cloned()
        .ok_or_else(|| format!("unknown {prefix} record: {name}"))?;
    let mut desc = String::new();
    desc.push_str(prefix);
    desc.push_str(":{");
    desc.push_str(&record_fields_descriptor(&rec, ctx)?);
    desc.push('}');
    Ok(desc)
}

/// Render one record's fields as `name:spec@off[:size]` — `rec:` fields
/// expand inline recursively; an unknown nested record degrades to an
/// opaque byte array (`arr:size:u8`) so the ABI stays correct.
fn record_fields_descriptor(
    rec: &handl_plugin_sdkgen::Record,
    ctx: &ParseCtx,
) -> Result<String, String> {
    let mut out = String::new();
    for (i, f) in rec.fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&f.name);
        out.push(':');
        let spec = if let Some(rn) = f.descriptor.strip_prefix("rec:") {
            let nested = ctx.records.borrow().get(rn).cloned();
            match nested {
                Some(n) => format!("rec:{{{}}}", record_fields_descriptor(&n, ctx)?),
                // Unknown nested record — opaque bytes keep the ABI right.
                None => format!("arr:{}:u8", f.size),
            }
        } else {
            f.descriptor.clone()
        };
        out.push_str(&spec);
        out.push('@');
        out.push_str(&f.offset.to_string());
        // Scalars carry width in their kind; pointer/array/nested slots
        // need the explicit byte width (pointer width is target-specific).
        if f.descriptor == "ptr"
            || f.descriptor.starts_with("arr:")
            || f.descriptor.starts_with("rec:")
        {
            out.push(':');
            out.push_str(&f.size.to_string());
        }
    }
    Ok(out)
}
