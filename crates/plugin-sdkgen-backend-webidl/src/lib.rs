//! WebIDL backend for the Handl SDK generator (ADR-0113).

use handl_plugin_sdkgen::{
    CEnumConst, CField, CFunction, CHandlArg, CRecord, CTypedef, SdkgenBackend, SdkgenError,
    SdkgenModule,
};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[allow(dead_code)]
mod parser;
use parser::{AstDef, Member, Type};

/// WebIDL parser backend.
pub struct WebIdlBackend {
    target: Option<String>,
}

impl WebIdlBackend {
    /// Create a new WebIDL backend with an optional target (defaults to `browser`).
    pub fn new(target: Option<String>) -> Self {
        Self { target }
    }

    fn default_target() -> String {
        "browser".into()
    }
}

impl SdkgenBackend for WebIdlBackend {
    fn parse_header(&self, path: &Path) -> Result<SdkgenModule, SdkgenError> {
        let src = std::fs::read_to_string(path)
            .map_err(|e| SdkgenError(format!("failed to read {}: {e}", path.display())))?;
        let (defs, mut diags) = parser::parse_webidl(&src);

        let mut module = SdkgenModule::default();
        module.client_package = "js".into();
        module.backend_id = "webidl".into();
        module.target = self
            .target
            .clone()
            .unwrap_or_else(WebIdlBackend::default_target);
        module.library = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module")
            .to_string();
        module.module = module.library.clone();

        // Merge partials of the same kind and name.
        let mut by_key: HashMap<(String, &'static str), AstDef> = HashMap::new();
        for def in defs {
            let key = (def.name().to_string(), def.kind());
            match by_key.get_mut(&key) {
                Some(existing) => existing.merge(def, &mut diags),
                None => {
                    by_key.insert(key, def);
                }
            }
        }

        let mut records: BTreeMap<String, CRecord> = BTreeMap::new();
        let mut functions: BTreeMap<(String, String), CFunction> = BTreeMap::new();
        let mut typedefs: BTreeMap<String, CTypedef> = BTreeMap::new();
        let mut enum_constants = Vec::new();

        for def in by_key.into_values() {
            match def {
                AstDef::Interface { name, members, .. }
                | AstDef::Mixin { name, members, .. }
                | AstDef::Dictionary { name, members, .. } => {
                    let mut fields = Vec::new();
                    for m in &members {
                        if let Some(field) = member_to_field(m) {
                            fields.push(field);
                        }
                    }
                    records.insert(
                        name.clone(),
                        CRecord {
                            name: name.clone(),
                            fields,
                            size: 0,
                            align: 0,
                            doc: None,
                        },
                    );
                    for m in &members {
                        if let Some(f) = member_to_function(&name, m) {
                            functions.insert((name.clone(), f.name.clone()), f);
                        }
                    }
                }
                AstDef::Namespace { name, members, .. } => {
                    for m in &members {
                        if let Some(f) = member_to_function(&name, m) {
                            functions.insert((name.clone(), f.name.clone()), f);
                        }
                    }
                }
                AstDef::Enum { name, values } => {
                    for (i, v) in values.iter().enumerate() {
                        enum_constants.push(CEnumConst {
                            name: format!("{name}_{}", to_pascal(v)),
                            value: i as i64,
                        });
                    }
                }
                AstDef::Typedef { name, ty } => {
                    let (port, descriptor) = map_type(&ty);
                    typedefs.insert(
                        name.clone(),
                        CTypedef {
                            name,
                            port,
                            descriptor,
                            doc: None,
                        },
                    );
                }
                AstDef::Callback {
                    name,
                    return_ty,
                    args,
                    ..
                } => {
                    if let Some(f) = callback_to_function(&name, &return_ty, &args) {
                        functions.insert((name.clone(), f.name.clone()), f);
                    }
                }
                AstDef::Skip(_) => {}
            }
        }

        module.records = records.into_values().collect();
        module.typedefs = typedefs.into_values().collect();
        module.functions = functions.into_values().collect();
        module.enum_constants = enum_constants;
        module.diagnostics = diags;
        Ok(module)
    }
}

fn member_to_field(m: &Member) -> Option<CField> {
    match m {
        Member::Attribute { name, ty, readonly } => {
            let (port, descriptor) = map_type(ty);
            Some(CField {
                name: name.clone(),
                port,
                descriptor,
                offset: 0,
                size: 0,
                ptr_depth: 0,
                is_const: *readonly,
            })
        }
        Member::DictionaryField { name, ty, .. } => {
            let (port, descriptor) = map_type(ty);
            Some(CField {
                name: name.clone(),
                port,
                descriptor,
                offset: 0,
                size: 0,
                ptr_depth: 0,
                is_const: false,
            })
        }
        _ => None,
    }
}

fn member_to_function(intf: &str, m: &Member) -> Option<CFunction> {
    let (op_name, return_ty, args) = match m {
        Member::Operation {
            name,
            return_ty,
            args,
        } => (name.clone(), return_ty, args),
        Member::Constructor { args } => ("constructor".into(), &Type::Void, args),
        _ => return None,
    };
    let snake = to_snake(&op_name);
    let full_name = format!("{}_{}", to_snake(intf), snake);
    let port_name = format!("{}{}", to_pascal(intf), to_pascal(&to_snake(&op_name)));
    let c_decl = format!("{}.{}", intf, op_name);
    Some(build_function(
        &full_name, port_name, return_ty, args, c_decl,
    ))
}

fn callback_to_function(
    name: &str,
    return_ty: &Type,
    args: &[parser::Argument],
) -> Option<CFunction> {
    let port_name = to_pascal(&to_snake(name));
    let c_decl = format!(
        "callback {}({})",
        name,
        args.iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    );
    Some(build_function(
        &to_snake(name),
        port_name,
        return_ty,
        args,
        c_decl,
    ))
}

fn build_function(
    name: &str,
    port_name: String,
    return_ty: &Type,
    args: &[parser::Argument],
    c_decl: String,
) -> CFunction {
    let mut port_args = Vec::new();
    let mut arg_descriptors = Vec::new();
    let mut variadic = false;
    for a in args {
        let (port, descriptor) = map_type(&a.ty);
        if a.variadic {
            variadic = true;
        }
        port_args.push(CHandlArg {
            name: a.name.clone(),
            port,
        });
        arg_descriptors.push(descriptor);
    }
    let (return_port, ret_descriptor) = map_type(return_ty);
    CFunction {
        name: name.to_string(),
        port_name,
        port_args,
        arg_descriptors,
        return_port,
        ret_descriptor,
        out_struct: None,
        variadic,
        c_decl,
        doc: None,
    }
}

fn map_type(t: &Type) -> (String, String) {
    match t {
        Type::Void | Type::Undefined => ("()".into(), "void".into()),
        Type::Bool => ("Bool".into(), "bool".into()),
        Type::I8 => ("Int".into(), "i8".into()),
        Type::U8 => ("Nat".into(), "u8".into()),
        Type::I16 => ("Int".into(), "i16".into()),
        Type::U16 => ("Nat".into(), "u16".into()),
        Type::I32 => ("Int".into(), "i32".into()),
        Type::U32 => ("Nat".into(), "u32".into()),
        Type::I64 => ("Int".into(), "i64".into()),
        Type::U64 => ("Nat".into(), "u64".into()),
        Type::F32 => ("Float".into(), "f32".into()),
        Type::F64 => ("Float".into(), "f64".into()),
        Type::String => ("String".into(), "js_string".into()),
        Type::Bytes => ("Bytes".into(), "js_bytes".into()),
        Type::Any
        | Type::Object
        | Type::Promise(_)
        | Type::Sequence(_)
        | Type::Record(_, _)
        | Type::Nullable(_)
        | Type::Union(_)
        | Type::Handle(_) => ("Handle".into(), "js_object".into()),
    }
}

fn to_snake(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c == '.' || c == '-' || c == ' ' || c == '_' {
            out.push('_');
        } else if c.is_uppercase() {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        } else {
            out.push(c);
        }
    }
    out.trim_matches('_').to_string()
}

fn to_pascal(s: &str) -> String {
    let mut out = String::new();
    let mut cap = true;
    for c in s.chars() {
        if c == '_' || c == '.' || c == '-' || c == ' ' {
            cap = true;
        } else if cap {
            for uc in c.to_uppercase() {
                out.push(uc);
            }
            cap = false;
        } else {
            out.push(c);
        }
    }
    out
}
