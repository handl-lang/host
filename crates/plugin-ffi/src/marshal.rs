//! Marshalling between `serde_json::Value` and C ABI types.
//!
//! The dispatcher marshals each handl argument to a C type per a descriptor
//! string, calls the C symbol, and then unmarshals the return value back to a
//! JSON value.
//!
//! ## Marshalling descriptors
//!
//! | Descriptor     | C type                  | JSON source          |
//! |----------------|-------------------------|----------------------|
//! | `"i32"`        | `int32_t` (value)       | number (range-checked) |
//! | `"i64"`        | `int64_t` (value)       | number               |
//! | `"f64"`        | `double` (value)        | number               |
//! | `"cstr"`       | `const char *`          | string (NUL-checked) |
//! | `"ptr"`        | `void *`                | number (address)     |
//! | `"handle"`     | `void *`                | `{"__resource__": {"ptr": N}}` (opaque handle, ADR-0083 §2.2) |
//! | `"void"`       | `void` (return)         | — (returns null)     |
//! | `"in_buf"`     | `const void * + size_t` | `Bytes`              |
//! | `"cbytes"`     | `const void * + size_t` | `Bytes`              |
//! | `"out_buf:N"`  | `void * + size_t`       | — (allocates N)      |
//! | `"out_ptr:N"`  | `void *`                | — (allocates N)      |
//! | `"io_buf:N"`   | `void * + size_t *`     | — (allocates N)      |
//!
//! `in_buf` / `cbytes` / `out_buf:N` / `out_ptr:N` / `io_buf:N` are the
//! buffer descriptors introduced with the `Bytes` type (ADR-0078 Phase 2 /
//! ADR-0083 §2.1). `in_buf` and `cbytes` each expand to **two** C
//! arguments — `(ptr, len)` — and consume one handl `Bytes` argument.
//! `out_buf:N` allocates `N` bytes, passes `(ptr, N)` as two C arguments,
//! consumes **no** handl argument, and produces an out-slot trimmed to the
//! `ret` byte count. `out_ptr:N` allocates `N` bytes and passes the bare
//! pointer as **one** C argument (e.g. `int sv[2]` in `socketpair`) — the
//! slot is the full `N` bytes. `io_buf:N` allocates an `N`-byte buffer
//! plus a `size_t` cell initialised to `N` and passes `(ptr, &len)` — the
//! POSIX value-result pattern (`sockaddr * + socklen_t *` in `accept`,
//! `getsockname`, `recvfrom`); the slot is trimmed to the length the
//! callee wrote back.
//!
//! The return becomes `{ ret, buf, outs }`: `buf` is the first out-slot
//! (kept for the common single-out case) and `outs` lists every out-slot
//! in descriptor order.
//!
//! `Bytes` values are carried as `serde_json::Value` using the
//! `{"__bytes__": [n, ...]}` encoding (matching `TaggedValue::Bytes` in
//! `handl-value`). For input, a plain JSON array of numbers is also accepted
//! as a convenience.

use std::ffi::{CStr, CString, c_char};

use handl_platform::EvalError;
use serde_json::{Map, Value as JsonValue};

// ---------------------------------------------------------------------------
// Intermediate C-side representations
// ---------------------------------------------------------------------------

/// A marshalled C argument ready to be passed across the FFI boundary.
///
/// All integer and pointer descriptors (`i32`, `i64`, `ptr`, `cstr`) are
/// widened to `i64`. This is sound on x86-64 SysV and AArch64 where a 32-bit
/// integer argument is sign/zero-extended in its calling register, so passing
/// the full 64-bit slot is equivalent. Floating-point values stay `f64`
/// because they travel in separate (XMM) registers and must not be
/// bit-cast to an integer slot.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CArg {
    /// An integer or pointer argument, widened to 64 bits.
    Int(i64),
    /// A double-precision floating-point argument.
    Float(f64),
}

/// The *kind* of return value expected from the C function.
///
/// This drives which function-pointer signature the dispatcher transmutes the
/// raw symbol to. Crucially, `Int32` and `Int64` produce *different* return
/// types in the transmuted signature: on x86-64 a function returning `int`
/// only writes the lower 32 bits of `rax` (the upper 32 are zeroed, **not**
/// sign-extended), so reading the full 64-bit `rax` as `i64` would turn a
/// `-1` `int` return into `4_294_967_295`. Reading it as `i32` and then
/// sign-extending to `i64` gives the correct `-1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CRetKind {
    /// 32-bit integer return (read as `i32`, sign-extended to `i64`).
    Int32,
    /// 64-bit integer or pointer return (read as `i64`).
    Int64,
    /// Double-precision floating-point return.
    Float,
    /// No return value.
    Void,
}

/// A value returned from a C function.
pub(crate) enum CRet {
    /// Integer or pointer return.
    Int(i64),
    /// Floating-point return.
    Float(f64),
    /// Void return.
    Void,
}

// ---------------------------------------------------------------------------
// Marshalling: JSON -> C
// ---------------------------------------------------------------------------

/// Marshal a single JSON argument to a [`CArg`] per its descriptor string.
///
/// For `cstr` descriptors the produced [`CString`] is pushed into
/// `keep_alive` so it lives until the C call returns; the returned [`CArg`]
/// only holds the raw pointer value.
pub(crate) fn marshal_arg(
    descriptor: &str,
    value: &JsonValue,
    keep_alive: &mut Vec<CString>,
) -> Result<CArg, EvalError> {
    match descriptor {
        "i32" => {
            let n = value
                .as_i64()
                .ok_or_else(|| EvalError::TypeMismatch("expected number for i32".to_string()))?;
            let v = i32::try_from(n)
                .map_err(|_| EvalError::TypeMismatch(format!("i32 overflow: {n}")))?;
            Ok(CArg::Int(v as i64))
        }
        "i64" => {
            let n = value
                .as_i64()
                .ok_or_else(|| EvalError::TypeMismatch("expected number for i64".to_string()))?;
            Ok(CArg::Int(n))
        }
        "f64" => {
            let n = value
                .as_f64()
                .ok_or_else(|| EvalError::TypeMismatch("expected number for f64".to_string()))?;
            Ok(CArg::Float(n))
        }
        "cstr" => {
            let s = value
                .as_str()
                .ok_or_else(|| EvalError::TypeMismatch("expected string for cstr".to_string()))?;
            let c = CString::new(s)
                .map_err(|_| EvalError::TypeMismatch("embedded NUL byte in cstr".to_string()))?;
            let ptr = c.as_ptr();
            keep_alive.push(c);
            Ok(CArg::Int(ptr as i64))
        }
        "ptr" => {
            let n = value
                .as_i64()
                .ok_or_else(|| EvalError::TypeMismatch("expected number for ptr".to_string()))?;
            Ok(CArg::Int(n))
        }
        // `handle` accepts a `{"__resource__": {"ptr": N}}` JSON value (the
        // debug form of `TaggedValue::Resource`) and passes the raw pointer
        // through to the C call. A bare number is also accepted for
        // convenience (e.g. a POSIX fd passed as an i32).
        "handle" => {
            if let Some(ptr_n) = value
                .as_object()
                .and_then(|obj| obj.get("__resource__"))
                .and_then(|r| r.get("ptr"))
                .and_then(|v| v.as_i64())
            {
                return Ok(CArg::Int(ptr_n));
            }
            let n = value.as_i64().ok_or_else(|| {
                EvalError::TypeMismatch("expected resource or number for handle".to_string())
            })?;
            Ok(CArg::Int(n))
        }
        other => Err(EvalError::TypeMismatch(format!(
            "unknown arg descriptor: {other}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Buffer descriptors: in_buf / cbytes / out_buf:N (ADR-0083 §2.1)
// ---------------------------------------------------------------------------

/// How an out-slot's bytes are trimmed when read back after the call.
pub(crate) enum OutTrim {
    /// First `ret` bytes of the buffer (`out_buf:N` — `recv`/`read` style,
    /// where the C return is the byte count). Empty when `ret < 0`.
    ByRet,
    /// The full `N` bytes (`out_ptr:N` — fixed-size out-params like
    /// `int sv[2]`).
    Full,
    /// First `*len_cell` bytes (`io_buf:N` — POSIX value-result, the
    /// callee writes the actual length back through the pointer).
    ByLenCell,
}

/// An allocated output buffer for `out_buf:N` / `out_ptr:N` / `io_buf:N`.
///
/// Owns the backing `Vec<u8>` (and the `size_t` cell for `io_buf`) so it
/// stays valid across the C call and is available for the post-call
/// `{ ret, buf, outs }` extraction.
pub(crate) struct OutSlot {
    /// The allocated buffer (zero-initialized). Valid until this is dropped.
    pub buf: Vec<u8>,
    /// The declared capacity `N` (== `buf.len()`).
    pub len: usize,
    /// Trim rule applied when reading `buf` back.
    pub trim: OutTrim,
    /// `io_buf` only: the `size_t` cell whose address is passed to C,
    /// initialised to `N` (capacity in, actual length out). Empty for the
    /// other kinds.
    pub len_cell: Vec<u8>,
}

/// An allocated C struct for an `out_struct:{...}` descriptor.
///
/// Owns the backing `Vec<u8>` so it stays valid across the C call and is
/// available for the post-call `{ ret, buf }` record extraction.
pub(crate) struct OutStruct {
    /// The allocated struct memory (zero-initialized). Valid until this is dropped.
    pub buf: Vec<u8>,
    /// Field metadata: (name, kind, offset, size).
    pub fields: Vec<(String, String, usize, usize)>,
}

/// The result of marshalling a whole argument list.
///
/// `c_args` is the flat list of C arguments to pass (buffer descriptors
/// expand to two entries each). `keep_alive` / `byte_keep_alive` keep the
/// backing storage alive for the duration of the C call. `out_bufs` records
/// any `out_buf:N` slots so the return can be wrapped as `{ ret, buf }`.
pub(crate) struct MarshalResult {
    pub c_args: Vec<CArg>,
    /// `CString`s owned by the marshaller (for `cstr` args).
    pub keep_alive: Vec<CString>,
    /// Source `Vec<u8>`s for `in_buf` / `cbytes` args (C reads from these).
    pub byte_keep_alive: Vec<Vec<u8>>,
    /// Allocated output buffers for `out_buf:N` / `out_ptr:N` / `io_buf:N`.
    pub out_slots: Vec<OutSlot>,
    /// Allocated C structs for `out_struct:{...}` descriptors.
    pub out_structs: Vec<OutStruct>,
    /// Built structs for `in_struct:{...}` / `io_struct:{...}` descriptors.
    /// `io_struct` entries are read back into records after the call.
    pub struct_args: Vec<StructArg>,
    /// Indexes into `struct_args` that are `io_struct` (read-back) entries.
    pub io_struct_idx: Vec<usize>,
}

/// Decode a JSON `Bytes` value into a `Vec<u8>`.
///
/// Accepts either the canonical `{"__bytes__": [n, ...]}` encoding (matching
/// `TaggedValue::Bytes`) or, as a convenience, a plain JSON array of numbers.
/// Each element must be an integer in `0..=255`; out-of-range or non-integer
/// elements are a marshal error rather than a silent truncation.
pub(crate) fn json_to_bytes(value: &JsonValue) -> Result<Vec<u8>, EvalError> {
    let arr: &[JsonValue] = if let Some(obj) = value.as_object() {
        match obj.get("__bytes__").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => {
                return Err(EvalError::TypeMismatch(
                    "expected Bytes ({__bytes__: [numbers]})".to_string(),
                ));
            }
        }
    } else if let Some(a) = value.as_array() {
        a
    } else {
        return Err(EvalError::TypeMismatch(
            "expected Bytes value (object or array)".to_string(),
        ));
    };
    let mut out = Vec::with_capacity(arr.len());
    for el in arr {
        let n = el.as_i64().ok_or_else(|| {
            EvalError::TypeMismatch("Bytes array must contain numbers".to_string())
        })?;
        let b = u8::try_from(n)
            .map_err(|_| EvalError::TypeMismatch(format!("byte out of range 0..=255: {n}")))?;
        out.push(b);
    }
    Ok(out)
}

/// Encode a byte slice as the canonical `{"__bytes__": [n, ...]}` JSON value.
pub(crate) fn bytes_to_json(bytes: &[u8]) -> JsonValue {
    let arr: Vec<JsonValue> = bytes.iter().map(|&b| JsonValue::from(b as i64)).collect();
    let mut map = Map::new();
    map.insert("__bytes__".to_string(), JsonValue::Array(arr));
    JsonValue::Object(map)
}

/// Marshal a full argument list, expanding buffer descriptors.
///
/// Unlike [`marshal_arg`] (which handles a single C argument), this walks the
/// whole descriptor list because `in_buf` / `cbytes` / `out_buf:N` each
/// expand to **two** C arguments. `out_buf:N` consumes no handl argument; the
/// other descriptors consume exactly one. The number of handl arguments must
/// equal the number of descriptors that consume one.
pub(crate) fn marshal_call_args(
    descriptors: &[String],
    args: &[JsonValue],
) -> Result<MarshalResult, EvalError> {
    let mut res = MarshalResult {
        c_args: Vec::with_capacity(descriptors.len() * 2),
        keep_alive: Vec::with_capacity(descriptors.len()),
        byte_keep_alive: Vec::with_capacity(descriptors.len()),
        out_slots: Vec::new(),
        out_structs: Vec::new(),
        struct_args: Vec::new(),
        io_struct_idx: Vec::new(),
    };
    let mut arg_idx = 0usize;
    for desc in descriptors {
        if desc == "in_buf" || desc == "cbytes" {
            let value = args
                .get(arg_idx)
                .ok_or_else(|| EvalError::TypeMismatch(format!("missing argument for {desc}")))?;
            arg_idx += 1;
            let bytes = json_to_bytes(value)?;
            let ptr = bytes.as_ptr() as i64;
            let len = bytes.len() as i64;
            // Move `bytes` into keep-alive *after* capturing the pointer; the
            // Vec's heap allocation is stable across the move.
            res.byte_keep_alive.push(bytes);
            res.c_args.push(CArg::Int(ptr));
            res.c_args.push(CArg::Int(len));
        } else if let Some(n_str) = desc.strip_prefix("out_buf:") {
            let n = parse_size("out_buf", n_str)?;
            let mut buf = vec![0u8; n];
            let ptr = buf.as_mut_ptr() as i64;
            res.c_args.push(CArg::Int(ptr));
            res.c_args.push(CArg::Int(n as i64));
            res.out_slots.push(OutSlot {
                buf,
                len: n,
                trim: OutTrim::ByRet,
                len_cell: Vec::new(),
            });
        } else if let Some(n_str) = desc.strip_prefix("out_ptr:") {
            let n = parse_size("out_ptr", n_str)?;
            let mut buf = vec![0u8; n];
            let ptr = buf.as_mut_ptr() as i64;
            res.c_args.push(CArg::Int(ptr));
            res.out_slots.push(OutSlot {
                buf,
                len: n,
                trim: OutTrim::Full,
                len_cell: Vec::new(),
            });
        } else if let Some(n_str) = desc.strip_prefix("io_buf:") {
            let n = parse_size("io_buf", n_str)?;
            let mut buf = vec![0u8; n];
            // Value-result cell: capacity in, actual length out.
            let mut len_cell = n.to_ne_bytes().to_vec();
            let ptr = buf.as_mut_ptr() as i64;
            let len_ptr = len_cell.as_mut_ptr() as i64;
            res.c_args.push(CArg::Int(ptr));
            res.c_args.push(CArg::Int(len_ptr));
            res.out_slots.push(OutSlot {
                buf,
                len: n,
                trim: OutTrim::ByLenCell,
                len_cell,
            });
        } else if let Some(inner) = desc
            .strip_prefix("in_struct:")
            .or_else(|| desc.strip_prefix("io_struct:"))
        {
            let is_io = desc.starts_with("io_struct:");
            let inner = inner
                .strip_prefix('{')
                .and_then(|s| s.strip_suffix('}'))
                .ok_or_else(|| {
                    EvalError::TypeMismatch(format!("struct descriptor must be {{...}}: {desc}"))
                })?;
            let record = if is_io {
                None
            } else {
                let value = args.get(arg_idx).ok_or_else(|| {
                    EvalError::TypeMismatch(format!("missing argument for {desc}"))
                })?;
                arg_idx += 1;
                Some(value)
            };
            let st = marshal_struct_arg(inner, record)?;
            let ptr = st.buf.as_ptr() as i64;
            res.c_args.push(CArg::Int(ptr));
            if is_io {
                res.io_struct_idx.push(res.struct_args.len());
            }
            res.struct_args.push(st);
        } else if desc.starts_with("out_struct:") {
            let out = parse_out_struct_descriptor(desc)?;
            let ptr = out.buf.as_ptr() as i64;
            res.c_args.push(CArg::Int(ptr));
            res.out_structs.push(out);
        } else {
            let value = args
                .get(arg_idx)
                .ok_or_else(|| EvalError::TypeMismatch(format!("missing argument for {desc}")))?;
            arg_idx += 1;
            let arg = marshal_arg(desc, value, &mut res.keep_alive)?;
            res.c_args.push(arg);
        }
    }
    if arg_idx != args.len() {
        return Err(EvalError::TypeMismatch(format!(
            "expected {arg_idx} handl args, got {}",
            args.len()
        )));
    }
    Ok(res)
}

fn parse_size(kind: &str, n_str: &str) -> Result<usize, EvalError> {
    n_str
        .parse()
        .map_err(|_| EvalError::TypeMismatch(format!("invalid {kind} size: {n_str:?}")))
}

/// Read an out-slot back as a `Bytes` JSON value, trimmed per its kind.
fn slot_json(s: &OutSlot, ret_i64: i64) -> JsonValue {
    let take = match s.trim {
        OutTrim::ByRet => {
            if ret_i64 < 0 {
                0
            } else {
                (ret_i64 as usize).min(s.len)
            }
        }
        OutTrim::Full => s.len,
        OutTrim::ByLenCell => {
            let n = usize::from_ne_bytes(
                s.len_cell
                    .as_slice()
                    .try_into()
                    .unwrap_or([0u8; std::mem::size_of::<usize>()]),
            );
            n.min(s.len)
        }
    };
    bytes_to_json(&s.buf[..take])
}

/// Wrap a raw C return as a `{ ret, buf, outs }` record when out-slots
/// were used.
///
/// `raw_ret` is the already-unmarshalled C return value. `outs` lists
/// every out-slot in descriptor order, each trimmed per its kind
/// (`ByRet` clamps to the return byte count, `Full` keeps all `N` bytes,
/// `ByLenCell` reads the callee's value-result length). `buf` aliases the
/// first slot for the common single-out case.
pub(crate) fn wrap_outs_return(raw_ret: &JsonValue, out_slots: &[OutSlot]) -> JsonValue {
    if out_slots.is_empty() {
        return raw_ret.clone();
    }
    let ret_i64 = raw_ret.as_i64().unwrap_or(0);
    let outs: Vec<JsonValue> = out_slots.iter().map(|s| slot_json(s, ret_i64)).collect();
    let mut map = Map::new();
    map.insert("ret".to_string(), raw_ret.clone());
    map.insert("buf".to_string(), outs[0].clone());
    map.insert("outs".to_string(), JsonValue::Array(outs));
    JsonValue::Object(map)
}

/// Parse an `out_struct:{field:desc,...}` descriptor and allocate its memory.
///
/// Computes a simple aligned layout for integer-typed fields (`i8`, `i16`, `i32`,
/// `i64`, `u8`, `u16`, `u32`, `u64`, `isize`, `usize`).
/// Fields are placed in declaration order with alignment equal to their size.
pub(crate) fn parse_out_struct_descriptor(desc: &str) -> Result<OutStruct, EvalError> {
    let inner = desc
        .strip_prefix("out_struct:")
        .ok_or_else(|| EvalError::TypeMismatch(format!("not an out_struct descriptor: {desc}")))?;
    if !inner.starts_with('{') || !inner.ends_with('}') {
        return Err(EvalError::TypeMismatch(format!(
            "out_struct descriptor must be {{...}}, got {inner}"
        )));
    }
    let content = &inner[1..inner.len() - 1];
    if content.is_empty() {
        return Ok(OutStruct {
            buf: Vec::new(),
            fields: Vec::new(),
        });
    }
    let mut fields = Vec::new();
    let mut offset = 0usize;
    for pair in content.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let mut parts = pair.splitn(2, ':');
        let name = parts
            .next()
            .ok_or_else(|| EvalError::TypeMismatch(format!("invalid out_struct field: {pair}")))?
            .trim()
            .to_string();
        let kind = parts
            .next()
            .ok_or_else(|| {
                EvalError::TypeMismatch(format!("out_struct field missing type: {pair}"))
            })?
            .trim()
            .to_string();
        let size = match kind.as_str() {
            "i8" | "u8" => 1usize,
            "i16" | "u16" => 2usize,
            "i32" | "u32" => 4usize,
            "i64" | "u64" | "isize" | "usize" => 8usize,
            other => {
                return Err(EvalError::TypeMismatch(format!(
                    "out_struct: unsupported field type {other}"
                )));
            }
        };
        let align = size;
        let misalign = offset % align;
        if misalign != 0 {
            offset += align - misalign;
        }
        fields.push((name, kind, offset, size));
        offset += size;
    }
    // Add trailing alignment to the struct's total size.
    let max_align = fields
        .iter()
        .map(|(_, _, _, size)| *size)
        .max()
        .unwrap_or(1);
    let total = if offset == 0 {
        0
    } else {
        let trailing = offset % max_align;
        if trailing == 0 {
            offset
        } else {
            offset + max_align - trailing
        }
    };
    Ok(OutStruct {
        buf: vec![0u8; total],
        fields,
    })
}

/// Read an already-called `OutStruct` back into a Handl record.
pub(crate) fn unmarshal_out_struct(out: &OutStruct) -> Result<JsonValue, EvalError> {
    let mut map = Map::new();
    for (name, kind, offset, size) in &out.fields {
        let bytes = out
            .buf
            .get(*offset..offset + size)
            .ok_or_else(|| EvalError::Other("out_struct field out of bounds".to_string()))?;
        let value = match kind.as_str() {
            "i8" => JsonValue::from(i8::from_ne_bytes([bytes[0]]) as i64),
            "u8" => JsonValue::from(u8::from_ne_bytes([bytes[0]]) as i64),
            "i16" => {
                let raw = <[u8; 2]>::try_from(bytes).map_err(|_| {
                    EvalError::Other("out_struct i16 field read failed".to_string())
                })?;
                JsonValue::from(i16::from_ne_bytes(raw) as i64)
            }
            "u16" => {
                let raw = <[u8; 2]>::try_from(bytes).map_err(|_| {
                    EvalError::Other("out_struct u16 field read failed".to_string())
                })?;
                JsonValue::from(u16::from_ne_bytes(raw) as i64)
            }
            "i32" => {
                let raw = <[u8; 4]>::try_from(bytes).map_err(|_| {
                    EvalError::Other("out_struct i32 field read failed".to_string())
                })?;
                JsonValue::from(i32::from_ne_bytes(raw) as i64)
            }
            "u32" => {
                let raw = <[u8; 4]>::try_from(bytes).map_err(|_| {
                    EvalError::Other("out_struct u32 field read failed".to_string())
                })?;
                JsonValue::from(u32::from_ne_bytes(raw) as i64)
            }
            "i64" => {
                let raw = <[u8; 8]>::try_from(bytes).map_err(|_| {
                    EvalError::Other("out_struct i64 field read failed".to_string())
                })?;
                JsonValue::from(i64::from_ne_bytes(raw))
            }
            "u64" => {
                let raw = <[u8; 8]>::try_from(bytes).map_err(|_| {
                    EvalError::Other("out_struct u64 field read failed".to_string())
                })?;
                JsonValue::from(u64::from_ne_bytes(raw) as i64)
            }
            "isize" => {
                let raw = <[u8; 8]>::try_from(bytes).map_err(|_| {
                    EvalError::Other("out_struct isize field read failed".to_string())
                })?;
                JsonValue::from(isize::from_ne_bytes(raw) as i64)
            }
            "usize" => {
                let raw = <[u8; 8]>::try_from(bytes).map_err(|_| {
                    EvalError::Other("out_struct usize field read failed".to_string())
                })?;
                JsonValue::from(usize::from_ne_bytes(raw) as i64)
            }
            other => {
                return Err(EvalError::TypeMismatch(format!(
                    "out_struct: unsupported field type {other}"
                )));
            }
        };
        map.insert(name.clone(), value);
    }
    Ok(JsonValue::Object(map))
}

/// Wrap a raw C return as a `{ ret, buf }` record when `out_struct` was
/// used. `buf` is the unmarshalled struct record. Any byte out-slots
/// (`out_buf`/`out_ptr`/`io_buf` in the same call) are additionally
/// listed under `outs` so they are not dropped.
pub(crate) fn wrap_out_struct_return(
    raw_ret: &JsonValue,
    out_structs: &[OutStruct],
    out_slots: &[OutSlot],
) -> JsonValue {
    let out = match out_structs.first() {
        Some(out) => out,
        None => return raw_ret.clone(),
    };
    let mut map = Map::new();
    map.insert("ret".to_string(), raw_ret.clone());
    map.insert(
        "buf".to_string(),
        unmarshal_out_struct(out).unwrap_or(JsonValue::Null),
    );
    if !out_slots.is_empty() {
        let ret_i64 = raw_ret.as_i64().unwrap_or(0);
        let outs: Vec<JsonValue> = out_slots.iter().map(|s| slot_json(s, ret_i64)).collect();
        map.insert("outs".to_string(), JsonValue::Array(outs));
    }
    JsonValue::Object(map)
}

// ---------------------------------------------------------------------------
// Struct descriptors: in_struct / io_struct
//
// `in_struct:{f:kind,...}` consumes one Handl record arg and *builds* a C
// struct whose pointer fields reference sibling buffers kept alive for the
// call — the `msghdr` pattern (`sendmsg`). `io_struct:{...}` consumes no
// Handl arg and allocates everything — the recv side (`recvmsg`) — and the
// fields are read back into a record after the call.
//
// Field kinds: scalar integers (`i8`..`u64`, `isize`, `usize`), `bytes`
// (Bytes arg → pointer), `iov` (List<Bytes> arg → iovec array), `buf:N`
// (allocate N bytes → pointer, read back as Bytes), `iovbuf:K:M` (allocate
// K buffers of M bytes + iovec array, read back as List<Bytes> trimmed by
// the cumulative `ret` byte count). A scalar field the record does not
// provide and whose name ends in `len` is auto-filled with the byte length
// / capacity / element count of the preceding reference field — the POSIX
// `msg_namelen`/`msg_iovlen`/`msg_controllen` convention.
// ---------------------------------------------------------------------------

/// Field kind inside an `in_struct:{...}` / `io_struct:{...}` descriptor.
#[derive(Debug)]
pub(crate) enum StructFieldKind {
    /// Integer scalar (`i8`..`u64`, `isize`, `usize`).
    Scalar(String),
    /// `bytes` — Handl record field is `Bytes`; address goes in the struct.
    BytesRef,
    /// `buf:N` — N-byte callee-filled buffer; pointer goes in the struct.
    Buf(usize),
    /// `iov` — Handl record field is `List<Bytes>`; builds an iovec array.
    IovRef,
    /// `iovbuf:K:M` — K callee-filled buffers of M bytes + iovec array.
    IovBuf(usize, usize),
}

/// One field of a struct descriptor.
pub(crate) struct StructField {
    pub name: String,
    pub kind: StructFieldKind,
    pub offset: usize,
    /// Index into [`StructArg::aux`] for reference kinds.
    pub aux: Option<usize>,
}

/// Buffers a struct field's pointer refers to.
pub(crate) struct FieldAux {
    /// `bytes`/`buf`: the single pointed-to buffer. `iov`/`iovbuf`: the
    /// per-element buffers.
    pub elems: Vec<Vec<u8>>,
    /// The iovec array for `iov`/`iovbuf` fields (empty otherwise).
    pub array: Vec<u8>,
}

/// A built C struct for `in_struct`/`io_struct`. Owns the struct bytes plus
/// every buffer its pointer fields reference — all kept alive until the C
/// call returns.
pub(crate) struct StructArg {
    /// The struct memory. Its address is the C argument.
    pub buf: Vec<u8>,
    pub fields: Vec<StructField>,
    pub aux: Vec<FieldAux>,
}

fn scalar_size(kind: &str) -> Result<usize, EvalError> {
    match kind {
        "i8" | "u8" => Ok(1),
        "i16" | "u16" => Ok(2),
        "i32" | "u32" => Ok(4),
        "i64" | "u64" | "isize" | "usize" => Ok(8),
        other => Err(EvalError::TypeMismatch(format!(
            "struct field: unsupported scalar {other}"
        ))),
    }
}

fn parse_field_kind(spec: &str) -> Result<StructFieldKind, EvalError> {
    if spec == "bytes" {
        return Ok(StructFieldKind::BytesRef);
    }
    if spec == "iov" {
        return Ok(StructFieldKind::IovRef);
    }
    if let Some(rest) = spec.strip_prefix("buf:") {
        return Ok(StructFieldKind::Buf(parse_size("buf", rest)?));
    }
    if let Some(rest) = spec.strip_prefix("iovbuf:") {
        let (k, m) = rest
            .split_once(':')
            .ok_or_else(|| EvalError::TypeMismatch(format!("iovbuf needs K:M, got {spec:?}")))?;
        return Ok(StructFieldKind::IovBuf(
            parse_size("iovbuf count", k)?,
            parse_size("iovbuf size", m)?,
        ));
    }
    scalar_size(spec).map(|_| StructFieldKind::Scalar(spec.to_string()))
}

fn field_width(kind: &StructFieldKind) -> usize {
    match kind {
        StructFieldKind::Scalar(s) => scalar_size(s).unwrap_or(8),
        _ => std::mem::size_of::<usize>(),
    }
}

fn write_int(buf: &mut [u8], offset: usize, size: usize, value: i64) {
    let bytes = value.to_ne_bytes();
    buf[offset..offset + size].copy_from_slice(&bytes[..size]);
}

fn read_int(buf: &[u8], offset: usize, size: usize) -> i64 {
    let mut tmp = [0u8; 8];
    tmp[..size].copy_from_slice(&buf[offset..offset + size]);
    i64::from_ne_bytes(tmp)
}

/// Parse an `in_struct:{...}`/`io_struct:{...}` descriptor into fields with
/// C-layout offsets (alignment == size, declaration order).
fn parse_struct_fields(inner: &str) -> Result<Vec<(String, StructFieldKind, usize)>, EvalError> {
    let mut fields = Vec::new();
    let mut offset = 0usize;
    for pair in inner.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let (name, spec) = pair
            .split_once(':')
            .ok_or_else(|| EvalError::TypeMismatch(format!("invalid struct field: {pair}")))?;
        let kind = parse_field_kind(spec.trim())?;
        let width = field_width(&kind);
        let misalign = offset % width;
        if misalign != 0 {
            offset += width - misalign;
        }
        fields.push((name.trim().to_string(), kind, offset));
        offset += width;
    }
    Ok(fields)
}

/// Build a C struct from a descriptor and (for `in_struct`) a Handl record.
fn marshal_struct_arg(inner: &str, record: Option<&JsonValue>) -> Result<StructArg, EvalError> {
    let parsed = parse_struct_fields(inner)?;
    let total = parsed
        .iter()
        .map(|(_, k, off)| off + field_width(k))
        .max()
        .unwrap_or(0);
    let mut arg = StructArg {
        buf: vec![0u8; total],
        fields: Vec::with_capacity(parsed.len()),
        aux: Vec::new(),
    };
    let obj = record.and_then(|v| v.as_object());
    let mut last_ref_len = 0usize;
    for (name, kind, offset) in parsed {
        let mut aux_idx = None;
        match &kind {
            StructFieldKind::Scalar(s) => {
                let size = scalar_size(s)?;
                let value = obj
                    .and_then(|o| o.get(&name))
                    .and_then(|v| v.as_i64())
                    .unwrap_or_else(|| {
                        // Auto-*len* fill: an absent scalar named `*len`
                        // takes the preceding reference field's length.
                        if name.ends_with("len") {
                            last_ref_len as i64
                        } else {
                            0
                        }
                    });
                write_int(&mut arg.buf, offset, size, value);
            }
            StructFieldKind::BytesRef => {
                let bytes = obj
                    .and_then(|o| o.get(&name))
                    .map(json_to_bytes)
                    .transpose()?;
                match bytes {
                    Some(bytes) if !bytes.is_empty() => {
                        last_ref_len = bytes.len();
                        let ptr = bytes.as_ptr() as i64;
                        arg.aux.push(FieldAux {
                            elems: vec![bytes],
                            array: Vec::new(),
                        });
                        aux_idx = Some(arg.aux.len() - 1);
                        write_int(&mut arg.buf, offset, 8, ptr);
                    }
                    // Absent/empty → NULL pointer, zero length.
                    _ => {
                        last_ref_len = 0;
                        arg.aux.push(FieldAux {
                            elems: Vec::new(),
                            array: Vec::new(),
                        });
                        aux_idx = Some(arg.aux.len() - 1);
                    }
                }
            }
            StructFieldKind::Buf(n) => {
                let buf = vec![0u8; *n];
                last_ref_len = *n;
                let ptr = buf.as_ptr() as i64;
                arg.aux.push(FieldAux {
                    elems: vec![buf],
                    array: Vec::new(),
                });
                aux_idx = Some(arg.aux.len() - 1);
                write_int(&mut arg.buf, offset, 8, ptr);
            }
            StructFieldKind::IovRef | StructFieldKind::IovBuf(_, _) => {
                let elems: Vec<Vec<u8>> = match &kind {
                    StructFieldKind::IovRef => obj
                        .and_then(|o| o.get(&name))
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().map(json_to_bytes).collect())
                        .transpose()?
                        .unwrap_or_default(),
                    StructFieldKind::IovBuf(k, m) => vec![vec![0u8; *m]; *k],
                    _ => unreachable!(),
                };
                last_ref_len = elems.len();
                let mut array = vec![0u8; elems.len() * 2 * 8];
                for (i, el) in elems.iter().enumerate() {
                    write_int(&mut array, i * 16, 8, el.as_ptr() as i64);
                    write_int(&mut array, i * 16 + 8, 8, el.len() as i64);
                }
                // Empty iov → NULL array pointer (Vec::as_ptr on an empty
                // Vec is dangling, not null).
                let ptr = if array.is_empty() {
                    0
                } else {
                    array.as_ptr() as i64
                };
                arg.aux.push(FieldAux { elems, array });
                aux_idx = Some(arg.aux.len() - 1);
                write_int(&mut arg.buf, offset, 8, ptr);
            }
        }
        arg.fields.push(StructField {
            name,
            kind,
            offset,
            aux: aux_idx,
        });
    }
    Ok(arg)
}

/// Read an `io_struct` back into a Handl record after the call.
///
/// Scalars read from the struct; `buf:N` fields become `Bytes` trimmed by
/// the following `*len` scalar's read-back value; `iovbuf` fields become
/// `List<Bytes>` trimmed by the cumulative `ret` count (the kernel fills
/// iovecs in order). `bytes`/`iov` input refs are omitted.
pub(crate) fn unmarshal_io_struct(arg: &StructArg, ret_i64: i64) -> JsonValue {
    let mut map = Map::new();
    // Scalars first — `buf` trims depend on their read-back values.
    let mut scalar_vals: Vec<(String, i64)> = Vec::new();
    for f in &arg.fields {
        if let StructFieldKind::Scalar(s) = &f.kind {
            let size = scalar_size(s).unwrap_or(8);
            scalar_vals.push((f.name.clone(), read_int(&arg.buf, f.offset, size)));
        }
    }
    let mut iov_credit = ret_i64.max(0) as usize;
    for (i, f) in arg.fields.iter().enumerate() {
        match &f.kind {
            StructFieldKind::Scalar(s) => {
                let size = scalar_size(s).unwrap_or(8);
                let v = read_int(&arg.buf, f.offset, size);
                map.insert(f.name.clone(), JsonValue::from(v));
            }
            StructFieldKind::Buf(_) => {
                let aux = &arg.aux[f.aux.unwrap_or(0)];
                let take = arg
                    .fields
                    .get(i + 1)
                    .filter(|n| matches!(n.kind, StructFieldKind::Scalar(_)))
                    .and_then(|n| {
                        scalar_vals
                            .iter()
                            .find(|(nm, _)| *nm == n.name)
                            .map(|(_, v)| *v)
                    })
                    .map(|v| (v.max(0) as usize).min(aux.elems[0].len()))
                    .unwrap_or_else(|| aux.elems[0].len());
                map.insert(f.name.clone(), bytes_to_json(&aux.elems[0][..take]));
            }
            StructFieldKind::IovBuf(_, _) => {
                let aux = &arg.aux[f.aux.unwrap_or(0)];
                let list: Vec<JsonValue> = aux
                    .elems
                    .iter()
                    .map(|el| {
                        let take = iov_credit.min(el.len());
                        iov_credit -= take;
                        bytes_to_json(&el[..take])
                    })
                    .collect();
                map.insert(f.name.clone(), JsonValue::Array(list));
            }
            _ => {}
        }
    }
    JsonValue::Object(map)
}

/// Wrap a raw C return as `{ ret, buf, outs }` when `io_struct` was used:
/// `buf` is the read-back struct record, `outs` any byte out-slots.
pub(crate) fn wrap_io_struct_return(
    raw_ret: &JsonValue,
    io_structs: &[&StructArg],
    out_slots: &[OutSlot],
) -> JsonValue {
    let st = match io_structs.first() {
        Some(st) => st,
        None => return raw_ret.clone(),
    };
    let ret_i64 = raw_ret.as_i64().unwrap_or(0);
    let mut map = Map::new();
    map.insert("ret".to_string(), raw_ret.clone());
    map.insert("buf".to_string(), unmarshal_io_struct(st, ret_i64));
    if !out_slots.is_empty() {
        let outs: Vec<JsonValue> = out_slots.iter().map(|s| slot_json(s, ret_i64)).collect();
        map.insert("outs".to_string(), JsonValue::Array(outs));
    }
    JsonValue::Object(map)
}

/// Returns `true` for descriptors that consume one Handl argument.
pub(crate) fn descriptor_consumes_handl_arg(desc: &str) -> bool {
    if desc == "void" {
        return false;
    }
    for prefix in [
        "out_buf:",
        "out_ptr:",
        "io_buf:",
        "out_struct:",
        "io_struct:",
    ] {
        if desc.starts_with(prefix) {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Unmarshalling: C -> JSON
// ---------------------------------------------------------------------------

/// Determine the return kind from a return descriptor string.
pub(crate) fn return_kind(descriptor: &str) -> Result<CRetKind, EvalError> {
    match descriptor {
        "i32" => Ok(CRetKind::Int32),
        "i64" | "ptr" | "cstr" | "handle" => Ok(CRetKind::Int64),
        "f64" => Ok(CRetKind::Float),
        "void" => Ok(CRetKind::Void),
        other => Err(EvalError::TypeMismatch(format!(
            "unknown return descriptor: {other}"
        ))),
    }
}

/// Unmarshal a C return value to a JSON value per the return descriptor.
pub(crate) fn unmarshal_return(descriptor: &str, ret: &CRet) -> Result<JsonValue, EvalError> {
    match descriptor {
        "i32" => match ret {
            CRet::Int(v) => Ok(JsonValue::from(*v as i32 as i64)),
            _ => Err(EvalError::Other(
                "expected integer return for i32".to_string(),
            )),
        },
        "i64" => match ret {
            CRet::Int(v) => Ok(JsonValue::from(*v)),
            _ => Err(EvalError::Other(
                "expected integer return for i64".to_string(),
            )),
        },
        "f64" => match ret {
            CRet::Float(v) => serde_json::Number::from_f64(*v)
                .map(JsonValue::Number)
                .ok_or_else(|| EvalError::Other("non-finite f64 return".to_string())),
            _ => Err(EvalError::Other(
                "expected float return for f64".to_string(),
            )),
        },
        "ptr" => match ret {
            CRet::Int(v) => Ok(JsonValue::from(*v)),
            _ => Err(EvalError::Other(
                "expected integer return for ptr".to_string(),
            )),
        },
        // `handle` wraps the raw C return pointer in the JSON debug form of
        // a `TaggedValue::Resource` (ADR-0083 §2.2): `{"__resource__":
        // {"ptr": N}}`. The finalizer cannot be carried across the JSON
        // boundary — when this JSON is decoded back to a `TaggedValue`, the
        // resulting `Resource` has `finalizer = None`. The `FfiOpFunction`
        // stores an optional finalizer field for future use once the
        // dispatcher returns `TaggedValue` directly instead of JSON.
        "handle" => match ret {
            CRet::Int(v) => {
                let mut inner = Map::new();
                inner.insert("ptr".to_string(), JsonValue::from(*v));
                let mut outer = Map::new();
                outer.insert("__resource__".to_string(), JsonValue::Object(inner));
                Ok(JsonValue::Object(outer))
            }
            _ => Err(EvalError::Other(
                "expected integer return for handle".to_string(),
            )),
        },
        "cstr" => match ret {
            CRet::Int(v) => {
                if *v == 0 {
                    return Ok(JsonValue::Null);
                }
                let ptr = *v as *const c_char;
                let s = unsafe { CStr::from_ptr(ptr) }
                    .to_str()
                    .map_err(|e| EvalError::Other(format!("non-utf8 cstr return: {e}")))?;
                Ok(JsonValue::from(s))
            }
            _ => Err(EvalError::Other(
                "expected integer return for cstr".to_string(),
            )),
        },
        "void" => Ok(JsonValue::Null),
        other => Err(EvalError::TypeMismatch(format!(
            "unknown return descriptor: {other}"
        ))),
    }
}
