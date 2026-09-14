//! C / libclang backend for the Handl SDK generator (ADR-0107).

#![allow(non_upper_case_globals)]

use handl_plugin_sdkgen::{
    CEnumConst, CField, CFunction, CHandlArg, CRecord, CTypedef, SdkgenBackend, SdkgenError,
    SdkgenModule,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::path::Path;

// libclang types and constants.
use clang_sys::*;

/// C header parser using libclang.
pub struct CBackend {
    target: Option<String>,
}

impl CBackend {
    pub fn new(target: Option<String>) -> Self {
        Self { target }
    }

    fn default_target() -> String {
        let arch = std::env::consts::ARCH;
        let os = std::env::consts::OS;
        let vendor = if os == "macos" { "apple" } else { "unknown" };
        format!("{arch}-{vendor}-{os}")
    }
}

fn resource_dir() -> Option<String> {
    std::process::Command::new("clang")
        .arg("-print-resource-dir")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

impl SdkgenBackend for CBackend {
    fn parse_header(&self, path: &Path) -> Result<SdkgenModule, SdkgenError> {
        clang_sys::load().map_err(|e| SdkgenError(format!("libclang load failed: {e}")))?;

        let target = self.target.clone().unwrap_or_else(CBackend::default_target);

        let index = unsafe { clang_createIndex(0, 0) };
        if index.is_null() {
            return Err(SdkgenError("libclang createIndex failed".into()));
        }

        let header_dir = path.parent().unwrap_or(Path::new("."));
        let mut raw_args: Vec<CString> = Vec::new();
        raw_args.push(CString::new("-x").unwrap());
        raw_args.push(CString::new("c").unwrap());
        let include = format!("-I{}", header_dir.display());
        raw_args.push(CString::new(include).unwrap());
        raw_args.push(CString::new("-I/usr/include").unwrap());
        if let Some(res) = resource_dir() {
            raw_args.push(CString::new("-resource-dir").unwrap());
            raw_args.push(CString::new(res).unwrap());
        }
        raw_args.push(CString::new("-target").unwrap());
        raw_args.push(CString::new(target.as_str()).unwrap());

        let args: Vec<*const c_char> = raw_args.iter().map(|s| s.as_ptr()).collect();

        let c_path = CString::new(path.to_string_lossy().as_bytes()).unwrap();
        let tu = unsafe {
            clang_parseTranslationUnit(
                index,
                c_path.as_ptr(),
                args.as_ptr(),
                args.len() as c_int,
                std::ptr::null_mut(),
                0,
                0,
            )
        };
        if tu.is_null() {
            unsafe { clang_disposeIndex(index) };
            return Err(SdkgenError("libclang failed to parse header".into()));
        }

        let mut fatal = false;
        let mut diag_msgs = Vec::new();
        let num = unsafe { clang_getNumDiagnostics(tu) };
        for i in 0..num {
            let diag = unsafe { clang_getDiagnostic(tu, i) };
            let sev = unsafe { clang_getDiagnosticSeverity(diag) };
            if sev == CXDiagnostic_Error || sev == CXDiagnostic_Fatal {
                fatal = true;
            }
            if sev >= CXDiagnostic_Warning {
                let sev_label = match sev {
                    CXDiagnostic_Warning => "warning",
                    CXDiagnostic_Error => "error",
                    CXDiagnostic_Fatal => "fatal",
                    _ => "note",
                };
                let s = unsafe { clang_getDiagnosticSpelling(diag) };
                let c = unsafe { clang_getCString(s) };
                if !c.is_null() {
                    let msg = unsafe { CStr::from_ptr(c).to_string_lossy().into_owned() };
                    diag_msgs.push(format!("[{sev_label}] {msg}"));
                }
                unsafe { clang_disposeString(s) };
            }
            unsafe { clang_disposeDiagnostic(diag) };
        }
        if fatal {
            unsafe { clang_disposeTranslationUnit(tu) };
            unsafe { clang_disposeIndex(index) };
            return Err(SdkgenError(format!(
                "libclang reported parse errors:\n{}",
                diag_msgs.join("\n")
            )));
        }

        let pointer_width_bits = unsafe {
            let ti = clang_getTranslationUnitTargetInfo(tu);
            let width = clang_TargetInfo_getPointerWidth(ti);
            clang_TargetInfo_dispose(ti);
            width as usize
        };
        let ctx = ParseCtx {
            pointer_width: pointer_width_bits,
            records: RefCell::new(HashMap::new()),
        };

        let mut cursor_data = CursorData::default();
        let root = unsafe { clang_getTranslationUnitCursor(tu) };
        unsafe {
            clang_visitChildren(
                root,
                collect_cursors,
                &mut cursor_data as *mut _ as *mut c_void,
            )
        };

        let mut module = SdkgenModule::default();
        module.client_package = "ffi".into();
        module.backend_id = "c".into();
        module.target = target;
        module.module = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module")
            .to_string();

        for &cursor in &cursor_data.records {
            match parse_record(cursor, &ctx) {
                Ok(rec) => {
                    ctx.records.borrow_mut().insert(rec.name.clone(), rec.clone());
                    module.records.push(rec);
                }
                Err(e) => module.diagnostics.push(e),
            }
        }

        for &cursor in &cursor_data.typedefs {
            match parse_typedef(cursor, &ctx) {
                Ok(td) => module.typedefs.push(td),
                Err(e) => module.diagnostics.push(e),
            }
        }

        for &cursor in &cursor_data.enums {
            collect_enum(cursor, &mut module);
        }

        for &cursor in &cursor_data.functions {
            match parse_function(cursor, &ctx) {
                Ok(f) => module.functions.push(f),
                Err(e) => module.diagnostics.push(e),
            }
        }

        unsafe {
            clang_disposeTranslationUnit(tu);
            clang_disposeIndex(index);
        }

        Ok(module)
    }
}

#[derive(Default)]
struct CursorData {
    functions: Vec<CXCursor>,
    records: Vec<CXCursor>,
    typedefs: Vec<CXCursor>,
    enums: Vec<CXCursor>,
}

struct ParseCtx {
    pointer_width: usize,
    records: std::cell::RefCell<HashMap<String, CRecord>>,
}

extern "C" fn collect_cursors(
    cursor: CXCursor,
    _parent: CXCursor,
    data: *mut c_void,
) -> CXChildVisitResult {
    let state = unsafe { &mut *(data as *mut CursorData) };
    if !from_main_file(cursor) {
        return CXChildVisit_Continue;
    }
    match cursor.kind {
        CXCursor_FunctionDecl => {
            if unsafe { clang_getCursorLinkage(cursor) } == CXLinkage_External {
                state.functions.push(cursor);
            }
        }
        CXCursor_StructDecl => {
            state.records.push(cursor);
        }
        CXCursor_TypedefDecl => {
            state.typedefs.push(cursor);
        }
        CXCursor_EnumDecl => {
            state.enums.push(cursor);
        }
        _ => {}
    }
    CXChildVisit_Continue
}

fn from_main_file(cursor: CXCursor) -> bool {
    unsafe { clang_Location_isFromMainFile(clang_getCursorLocation(cursor)) != 0 }
}

fn cursor_name(cursor: CXCursor) -> String {
    unsafe { cx_string(clang_getCursorSpelling(cursor)) }
}

fn type_spelling(ty: CXType) -> String {
    unsafe { cx_string(clang_getTypeSpelling(ty)) }
}

fn cx_string(str: CXString) -> String {
    unsafe {
        let ptr = clang_getCString(str);
        let s = if ptr.is_null() {
            String::new()
        } else {
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        };
        clang_disposeString(str);
        s
    }
}

fn parse_record(cursor: CXCursor, _ctx: &ParseCtx) -> Result<CRecord, String> {
    let name = cursor_name(cursor);
    if name.is_empty() {
        return Err("anonymous struct is not supported".into());
    }
    let ty = unsafe { clang_getCursorType(cursor) };
    let size = unsafe { clang_Type_getSizeOf(ty) };
    let align = unsafe { clang_Type_getAlignOf(ty) };
    if size < 0 || align < 0 {
        return Err(format!("struct {name} has incomplete layout"));
    }

    let mut fields = Vec::new();
    let mut field_cursors = Vec::new();
    unsafe {
        clang_visitChildren(
            cursor,
            field_collector,
            &mut field_cursors as *mut _ as *mut c_void,
        )
    };
    for f in field_cursors {
        match parse_field(f) {
            Ok(field) => fields.push(field),
            Err(e) => return Err(format!("struct {name}: {e}")),
        }
    }

    Ok(CRecord {
        name,
        fields,
        size: size as usize,
        align: align as usize,
    })
}

extern "C" fn field_collector(
    cursor: CXCursor,
    _parent: CXCursor,
    data: *mut c_void,
) -> CXChildVisitResult {
    if cursor.kind == CXCursor_FieldDecl {
        let fields = unsafe { &mut *(data as *mut Vec<CXCursor>) };
        fields.push(cursor);
    }
    CXChildVisit_Continue
}

fn parse_field(cursor: CXCursor) -> Result<CField, String> {
    let name = cursor_name(cursor);
    let ty = unsafe { clang_getCursorType(cursor) };
    let offset_bits = unsafe { clang_Cursor_getOffsetOfField(cursor) };
    let size = unsafe { clang_Type_getSizeOf(ty) };
    if size < 0 {
        return Err(format!("field {name} has unknown size"));
    }
    let (port, descriptor) = map_field_type(ty, size as usize)?;
    Ok(CField {
        name,
        port,
        descriptor,
        offset: (offset_bits / 8) as usize,
        size: size as usize,
        ptr_depth: 0,
        is_const: unsafe { clang_isConstQualifiedType(ty) != 0 },
    })
}

fn parse_typedef(cursor: CXCursor, _ctx: &ParseCtx) -> Result<CTypedef, String> {
    let name = cursor_name(cursor);
    if name.is_empty() {
        return Err("anonymous typedef is not supported".into());
    }
    let underlying = unsafe { clang_getTypedefDeclUnderlyingType(cursor) };
    let (port, descriptor) = map_arg_type(underlying, 64)?; // default width for typedefs
    Ok(CTypedef {
        name,
        port,
        descriptor,
    })
}

fn collect_enum(cursor: CXCursor, module: &mut SdkgenModule) {
    let mut constants = Vec::new();
    unsafe {
        clang_visitChildren(
            cursor,
            enum_collector,
            &mut constants as *mut _ as *mut c_void,
        )
    };
    for c in constants {
        let name = cursor_name(c);
        if name.is_empty() {
            continue;
        }
        let value = unsafe { clang_getEnumConstantDeclValue(c) };
        module.enum_constants.push(CEnumConst {
            name,
            value,
        });
    }
}

extern "C" fn enum_collector(
    cursor: CXCursor,
    _parent: CXCursor,
    data: *mut c_void,
) -> CXChildVisitResult {
    if cursor.kind == CXCursor_EnumConstantDecl {
        let v = unsafe { &mut *(data as *mut Vec<CXCursor>) };
        v.push(cursor);
    }
    CXChildVisit_Continue
}

fn parse_function(cursor: CXCursor, ctx: &ParseCtx) -> Result<CFunction, String> {
    let name = cursor_name(cursor);
    if name.is_empty() {
        return Err("unnamed function".into());
    }
    let port_name = to_pascal(&name);
    let c_decl = type_spelling(unsafe { clang_getCursorType(cursor) });

    let n_args = unsafe { clang_Cursor_getNumArguments(cursor) };
    if n_args < 0 {
        return Err(format!("function {name} argument count unknown"));
    }
    let n = n_args as usize;
    let mut cursors = Vec::with_capacity(n);
    for i in 0..n {
        cursors.push(unsafe { clang_Cursor_getArgument(cursor, i as c_uint) });
    }

    let ret_type = unsafe { clang_getCursorResultType(cursor) };
    let variadic = unsafe { clang_isFunctionTypeVariadic(clang_getCursorType(cursor)) != 0 };

    let mut port_args = Vec::new();
    let mut arg_descriptors = Vec::new();
    let mut out_struct: Option<String> = None;

    let mut i = 0;
    while i < cursors.len() {
        let arg = cursors[i];
        let arg_name = cursor_name(arg);
        let arg_ty = unsafe { clang_getCursorType(arg) };
        let next = cursors.get(i + 1).copied();

        let info = map_arg_with_peek(arg_ty, next, ctx)?;
        if info.is_size_pair {
            i += 2;
            continue;
        }

        if info.is_out_struct {
            let record = out_struct_descriptor(&info.record_name, ctx)?;
            arg_descriptors.push(record);
            out_struct = info.record_name.clone();
            i += 1;
            continue;
        }

        if info.consumes {
            port_args.push(CHandlArg {
                name: arg_name,
                port: info.port,
            });
        }
        arg_descriptors.push(info.descriptor);
        i += 1;
    }

    if variadic {
        // Variadic tails are not passed to ffi.call.
    }

    let (ret_port, ret_descriptor) = map_return_type(ret_type, ctx)?;
    let return_port = out_struct.clone().unwrap_or(ret_port);

    Ok(CFunction {
        name,
        port_name,
        port_args,
        arg_descriptors,
        return_port,
        ret_descriptor,
        out_struct,
        variadic,
        c_decl,
    })
}

struct ArgInfo {
    port: String,
    descriptor: String,
    consumes: bool,
    is_out_struct: bool,
    record_name: Option<String>,
    is_size_pair: bool,
}

fn map_arg_with_peek(
    ty: CXType,
    next: Option<CXCursor>,
    ctx: &ParseCtx,
) -> Result<ArgInfo, String> {
    let original = type_spelling(ty);
    let canonical = unsafe { clang_getCanonicalType(ty) };

    // size_t / ssize_t
    if original == "size_t" || original == "ssize_t" {
        let port = if original == "size_t" { "Nat" } else { "Int" };
        let desc = if ctx.pointer_width == 32 { "i32" } else { "i64" };
        return Ok(ArgInfo {
            port: port.into(),
            descriptor: desc.into(),
            consumes: true,
            is_out_struct: false,
            record_name: None,
            is_size_pair: false,
        });
    }

    if canonical.kind == CXType_Pointer {
        let pointee = unsafe { clang_getPointeeType(canonical) };
        let const_pointee = unsafe { clang_isConstQualifiedType(pointee) != 0 };
        let pointee_canonical = unsafe { clang_getCanonicalType(pointee) };

        // const void* / const char* with size_t -> in_buf / cbytes
        if const_pointee && is_void(pointee_canonical) {
            if let Some(next_c) = next {
                let next_ty = unsafe { clang_getCursorType(next_c) };
                if is_size_t(next_ty) {
                    return Ok(ArgInfo {
                        port: "Bytes".into(),
                        descriptor: "in_buf".into(),
                        consumes: true,
                        is_out_struct: false,
                        record_name: None,
                        is_size_pair: true,
                    });
                }
            }
        }
        if const_pointee && is_char(pointee_canonical) {
            if let Some(next_c) = next {
                let next_ty = unsafe { clang_getCursorType(next_c) };
                if is_size_t(next_ty) {
                    return Ok(ArgInfo {
                        port: "Bytes".into(),
                        descriptor: "cbytes".into(),
                        consumes: true,
                        is_out_struct: false,
                        record_name: None,
                        is_size_pair: true,
                    });
                }
            }
            // Fall back to cstr if no size is present.
            return Ok(ArgInfo {
                port: "String".into(),
                descriptor: "cstr".into(),
                consumes: true,
                is_out_struct: false,
                record_name: None,
                is_size_pair: false,
            });
        }

        // char* (NUL string)
        if is_char(pointee_canonical) {
            return Ok(ArgInfo {
                port: "String".into(),
                descriptor: "cstr".into(),
                consumes: true,
                is_out_struct: false,
                record_name: None,
                is_size_pair: false,
            });
        }

        // void* (opaque handle / ptr)
        if is_void(pointee_canonical) && !const_pointee {
            return Ok(ArgInfo {
                port: "Handle".into(),
                descriptor: "ptr".into(),
                consumes: true,
                is_out_struct: false,
                record_name: None,
                is_size_pair: false,
            });
        }

        // non-const struct* -> out_struct
        if pointee_canonical.kind == CXType_Record && !const_pointee {
            let record = record_name(pointee_canonical)?;
            return Ok(ArgInfo {
                port: record.clone(),
                descriptor: "out_struct".into(),
                consumes: false,
                is_out_struct: true,
                record_name: Some(record),
                is_size_pair: false,
            });
        }

        return Err(format!("unsupported pointer type: {}", type_spelling(ty)));
    }

    let (port, descriptor) = map_arg_type(canonical, ctx.pointer_width)?;
    Ok(ArgInfo {
        port,
        descriptor,
        consumes: true,
        is_out_struct: false,
        record_name: None,
        is_size_pair: false,
    })
}

fn map_arg_type(ty: CXType, pointer_width: usize) -> Result<(String, String), String> {
    let spelling = type_spelling(ty);
    if spelling == "size_t" {
        return Ok(("Nat".into(), if pointer_width == 32 { "i32" } else { "i64" }.into()));
    }
    if spelling == "ssize_t" {
        return Ok(("Int".into(), if pointer_width == 32 { "i32" } else { "i64" }.into()));
    }

    match ty.kind {
        CXType_Int => Ok(("Int".into(), "i32".into())),
        CXType_Long | CXType_LongLong => Ok(("Int".into(), "i64".into())),
        CXType_UInt | CXType_ULong | CXType_ULongLong => {
            // unsigned 32/64-bit; marshal as i32/i64
            let size = unsafe { clang_Type_getSizeOf(ty) };
            if size == 4 {
                Ok(("Nat".into(), "i32".into()))
            } else {
                Ok(("Nat".into(), "i64".into()))
            }
        }
        CXType_Double => Ok(("Float".into(), "f64".into())),
        CXType_Enum => Ok(("Int".into(), "i32".into())),
        CXType_Typedef => map_arg_type(unsafe { clang_getCanonicalType(ty) }, pointer_width),
        _ => Err(format!("unsupported argument type: {}", type_spelling(ty))),
    }
}

fn map_field_type(ty: CXType, _size: usize) -> Result<(String, String), String> {
    let spelling = type_spelling(ty);
    if spelling == "size_t" {
        return Ok(("Nat".into(), "usize".into()));
    }
    if spelling == "ssize_t" {
        return Ok(("Int".into(), "isize".into()));
    }

    match ty.kind {
        CXType_SChar | CXType_Char_S => Ok(("Int".into(), "i8".into())),
        CXType_UChar | CXType_Char_U => Ok(("Nat".into(), "u8".into())),
        CXType_Short => Ok(("Int".into(), "i16".into())),
        CXType_UShort => Ok(("Nat".into(), "u16".into())),
        CXType_Int => Ok(("Int".into(), "i32".into())),
        CXType_UInt => Ok(("Nat".into(), "u32".into())),
        CXType_Long | CXType_LongLong => Ok(("Int".into(), "i64".into())),
        CXType_ULong | CXType_ULongLong => Ok(("Nat".into(), "u64".into())),
        CXType_Double => Ok(("Float".into(), "f64".into())),
        CXType_Enum => Ok(("Int".into(), "i32".into())),
        CXType_Typedef => map_field_type(unsafe { clang_getCanonicalType(ty) }, _size),
        _ => Err(format!("unsupported field type: {}", type_spelling(ty))),
    }
}

fn map_return_type(ty: CXType, ctx: &ParseCtx) -> Result<(String, String), String> {
    if ty.kind == CXType_Void {
        return Ok(("()".into(), "void".into()));
    }
    if ty.kind == CXType_Pointer {
        let pointee = unsafe { clang_getPointeeType(ty) };
        let pointee_canonical = unsafe { clang_getCanonicalType(pointee) };
        if is_void(pointee_canonical) {
            return Ok(("Handle".into(), "handle".into()));
        }
    }
    let (port, descriptor) = map_arg_type(unsafe { clang_getCanonicalType(ty) }, ctx.pointer_width)?;
    Ok((port, descriptor))
}

fn out_struct_descriptor(name: &Option<String>, ctx: &ParseCtx) -> Result<String, String> {
    let name = name.as_ref().ok_or("missing out-struct name")?;
    let rec = ctx
        .records
        .borrow()
        .get(name)
        .cloned()
        .ok_or_else(|| format!("unknown out-struct record: {name}"))?;
    let mut desc = String::new();
    desc.push_str("out_struct:{");
    for (i, f) in rec.fields.iter().enumerate() {
        if i > 0 {
            desc.push(',');
        }
        desc.push_str(&f.name);
        desc.push(':');
        desc.push_str(&f.descriptor);
    }
    desc.push('}');
    Ok(desc)
}

fn record_name(ty: CXType) -> Result<String, String> {
    let decl = unsafe { clang_getTypeDeclaration(ty) };
    let name = cursor_name(decl);
    if name.is_empty() {
        Err(format!("unnamed record: {}", type_spelling(ty)))
    } else {
        Ok(name)
    }
}

fn is_void(ty: CXType) -> bool {
    ty.kind == CXType_Void
}

fn is_char(ty: CXType) -> bool {
    matches!(ty.kind, CXType_Char_S | CXType_UChar | CXType_Char_U | CXType_SChar)
}

fn is_size_t(ty: CXType) -> bool {
    let spelling = type_spelling(ty);
    spelling == "size_t"
}

fn to_pascal(name: &str) -> String {
    name.split('_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
            }
        })
        .collect()
}
