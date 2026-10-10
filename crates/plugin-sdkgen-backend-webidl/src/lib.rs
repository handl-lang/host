//! WebIDL backend for the Handl SDK generator (ADR-0113).

use handl_plugin_sdkgen::{
    Arg, DocSource, EnumConst, Field, Function, Record, SdkgenBackend, SdkgenError, SdkgenModule,
    Typedef, pick_doc,
};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

mod config;
mod mdn;
mod parser;

pub use config::WebIdlConfig;
pub use mdn::MdnDocSource;
use parser::{AstDef, Member, Type};

/// WebIDL parser backend.
pub struct WebIdlBackend {
    target: Option<String>,
    /// Supplementary spec files whose `interface mixin` definitions are
    /// visible to `X includes Y` resolution. Their own interfaces,
    /// records, typedefs, and constants are not emitted.
    include_specs: Vec<PathBuf>,
}

impl WebIdlBackend {
    /// Create a new WebIDL backend with an optional target (defaults to `browser`).
    pub fn new(target: Option<String>) -> Self {
        Self {
            target,
            include_specs: Vec::new(),
        }
    }

    /// Register a supplementary spec for cross-spec mixin resolution.
    pub fn add_include_spec(&mut self, path: impl Into<PathBuf>) {
        self.include_specs.push(path.into());
    }

    fn default_target() -> String {
        "browser".into()
    }

    /// Layer `docs:` overrides and an optional external [`DocSource`]
    /// (e.g. [`MdnDocSource`]) over comments captured from the spec. Per
    /// slot: override > in-source comment > external lookup. Keys are
    /// `Interface.member` c_decls (snake `interface_member` and port
    /// `InterfaceMember` spellings also match overrides), `Interface`,
    /// `Interface.field`, and typedef/constant names.
    ///
    /// When `external_docs` is not `false` (the default) and no `source`
    /// is supplied, an [`MdnDocSource`] is auto-created and consulted.
    pub fn apply_docs(
        &self,
        module: &mut SdkgenModule,
        config: &WebIdlConfig,
        source: Option<&dyn DocSource>,
    ) {
        let docs = &config.docs;
        // Respect `external_docs: false` — skip external lookup entirely.
        // Otherwise auto-create an MDN source if the caller gave none.
        let fallback = if config.external_docs != Some(false) && source.is_none() {
            Some(MdnDocSource::new())
        } else {
            None
        };
        let src: Option<&dyn DocSource> = if config.external_docs == Some(false) {
            None
        } else {
            source.or_else(|| fallback.as_ref().map(|s| s as &dyn DocSource))
        };
        for f in &mut module.functions {
            // `Intf.member` c_decls map to MDN member pages; callback
            // c_decls (`callback Name(...)`) have no page.
            let external =
                (f.c_decl.contains('.') && !f.c_decl.contains(' ')).then_some(f.c_decl.as_str());
            pick_doc(
                &mut f.doc,
                docs,
                &[&f.c_decl, &f.name, &f.port_name],
                src,
                external,
            );
        }
        for r in &mut module.records {
            pick_doc(&mut r.doc, docs, &[&r.name], src, Some(&r.name));
            for f in &mut r.fields {
                let key = format!("{}.{}", r.name, f.name);
                pick_doc(&mut f.doc, docs, &[&key], src, Some(&key));
            }
        }
        for t in &mut module.typedefs {
            pick_doc(&mut t.doc, docs, &[&t.name], None, None);
        }
        for c in &mut module.enum_constants {
            pick_doc(&mut c.doc, docs, &[&c.name], None, None);
        }
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
        // WebIDL records never reference each other by name (foreign
        // types map to `Handle`), so each interface can be its own file.
        module.per_record_files = true;

        // `A includes B;` pairs graft the mixin's members onto the
        // target interface below; they aren't definitions themselves.
        let mut includes: Vec<(String, String)> = Vec::new();
        // Merge partials of the same kind and name.
        let mut by_key: HashMap<(String, &'static str), AstDef> = HashMap::new();
        for def in defs {
            if let AstDef::Includes { target, mixin } = &def {
                includes.push((target.clone(), mixin.clone()));
                continue;
            }
            let key = (def.name().to_string(), def.kind());
            match by_key.get_mut(&key) {
                Some(existing) => existing.merge(def, &mut diags),
                None => {
                    by_key.insert(key, def);
                }
            }
        }

        // Graft each mixin's members onto its includers. Members the
        // target already declares are not duplicated.
        let mut mixin_members: HashMap<String, Vec<Member>> = by_key
            .values()
            .filter_map(|d| match d {
                AstDef::Mixin { name, members, .. } => Some((name.clone(), members.clone())),
                _ => None,
            })
            .collect();
        // Supplementary specs contribute mixin definitions *and* graft
        // targets: an `includes` whose interface lives in another spec
        // (storage.idl's `Navigator includes NavigatorStorage`, where
        // `Navigator` is html.idl's) grafts onto the foreign interface and
        // emits the augmented record in this package. Primary-spec defs
        // win on a name collision.
        let mut include_interfaces: HashMap<String, Vec<Member>> = HashMap::new();
        let mut include_includes: Vec<(String, String)> = Vec::new();
        for inc in &self.include_specs {
            match std::fs::read_to_string(inc) {
                Ok(src) => {
                    let (inc_defs, inc_diags) = parser::parse_webidl(&src);
                    for d in inc_diags {
                        diags.push(format!("{}: {d}", inc.display()));
                    }
                    for d in inc_defs {
                        match d {
                            AstDef::Mixin { name, members, .. } => {
                                mixin_members.entry(name).or_insert(members);
                            }
                            // Merge partials like the primary `by_key` pass —
                            // `interface Navigator` + `partial interface
                            // Navigator` contribute one member set.
                            AstDef::Interface { name, members, .. } => {
                                let entry = include_interfaces.entry(name).or_default();
                                for m in members {
                                    let dup = member_name(&m).is_some_and(|n| {
                                        entry.iter().any(|e| member_name(e) == Some(n))
                                    });
                                    if !dup {
                                        entry.push(m);
                                    }
                                }
                            }
                            // The supplementary spec's own augmentations —
                            // html.idl builds `Navigator` almost entirely
                            // through `Navigator includes NavigatorID` etc.
                            AstDef::Includes { target, mixin } => {
                                include_includes.push((target, mixin));
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => diags.push(format!(
                    "failed to read include spec {}: {e}",
                    inc.display()
                )),
            }
        }
        // Graft supplementary-spec `includes` onto supplementary interfaces so
        // foreign targets arrive complete (NavigatorID/Language/… onto
        // Navigator) before the primary pass consumes them.
        for (target, mixin) in include_includes {
            let (Some(members), Some(target_members)) = (
                mixin_members.get(&mixin),
                include_interfaces.get_mut(&target),
            ) else {
                continue;
            };
            for m in members {
                let dup = member_name(m)
                    .is_some_and(|n| target_members.iter().any(|t| member_name(t) == Some(n)));
                if !dup {
                    target_members.push(m.clone());
                }
            }
        }
        for (target, mixin) in includes {
            let Some(members) = mixin_members.get(&mixin) else {
                diags.push(format!("includes: mixin {mixin} not found (on {target})"));
                continue;
            };
            // Foreign target: pull the interface from an include spec into
            // `by_key` so the augmented record emits in this package.
            if !by_key.contains_key(&(target.clone(), "interface")) {
                if let Some(inc_members) = include_interfaces.get(&target) {
                    by_key.insert(
                        (target.clone(), "interface"),
                        AstDef::Interface {
                            name: target.clone(),
                            base: None,
                            members: inc_members.clone(),
                            partial: false,
                            doc: None,
                        },
                    );
                }
            }
            let Some(target_members) = by_key
                .get_mut(&(target.clone(), "interface"))
                .and_then(AstDef::members_mut)
            else {
                diags.push(format!(
                    "includes: {target} is not an interface in this spec"
                ));
                continue;
            };
            for m in members {
                let dup = member_name(m)
                    .is_some_and(|n| target_members.iter().any(|t| member_name(t) == Some(n)));
                if !dup {
                    target_members.push(m.clone());
                }
            }
        }

        let mut records: BTreeMap<String, Record> = BTreeMap::new();
        let mut functions: BTreeMap<(String, String), Function> = BTreeMap::new();
        let mut typedefs: BTreeMap<String, Typedef> = BTreeMap::new();
        let mut enum_constants = Vec::new();

        for def in by_key.into_values() {
            match def {
                AstDef::Interface {
                    name, members, doc, ..
                }
                | AstDef::Mixin {
                    name, members, doc, ..
                }
                | AstDef::Dictionary {
                    name, members, doc, ..
                } => {
                    let mut fields = Vec::new();
                    for m in &members {
                        if let Some(field) = member_to_field(m) {
                            fields.push(field);
                        }
                    }
                    records.insert(
                        name.clone(),
                        Record {
                            name: name.clone(),
                            fields,
                            size: 0,
                            align: 0,
                            doc,
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
                AstDef::Enum { name, values, .. } => {
                    for (i, v) in values.iter().enumerate() {
                        enum_constants.push(EnumConst {
                            name: format!("{name}_{}", to_pascal(&v.name)),
                            value: i as i64,
                            doc: v.doc.clone(),
                        });
                    }
                }
                AstDef::Typedef { name, ty, doc } => {
                    let (port, descriptor) = map_type(&ty);
                    typedefs.insert(
                        name.clone(),
                        Typedef {
                            name,
                            port,
                            descriptor,
                            doc,
                        },
                    );
                }
                AstDef::Callback {
                    name,
                    return_ty,
                    args,
                    doc,
                    ..
                } => {
                    if let Some(f) = callback_to_function(&name, &return_ty, &args, doc) {
                        functions.insert((name.clone(), f.name.clone()), f);
                    }
                }
                AstDef::Includes { .. } | AstDef::Skip(_) => {}
            }
        }

        module.records = records.into_values().collect();
        module.typedefs = typedefs.into_values().collect();
        module.functions = functions.into_values().collect();
        enum_constants.sort_by(|a, b| a.name.cmp(&b.name));
        module.enum_constants = enum_constants;
        module.diagnostics = diags;
        Ok(module)
    }
}

/// The declared name of a member, when it has one (constructors and
/// skipped members are anonymous).
fn member_name(m: &Member) -> Option<&str> {
    match m {
        Member::Const { name, .. }
        | Member::Attribute { name, .. }
        | Member::DictionaryField { name, .. }
        | Member::Operation { name, .. } => Some(name),
        Member::Constructor { .. } | Member::Skip(_) => None,
    }
}

fn member_to_field(m: &Member) -> Option<Field> {
    match m {
        Member::Attribute {
            name,
            ty,
            readonly,
            doc,
        } => {
            let (port, descriptor) = map_type(ty);
            Some(Field {
                name: name.clone(),
                port,
                descriptor,
                offset: 0,
                size: 0,
                ptr_depth: 0,
                is_const: *readonly,
                doc: doc.clone(),
            })
        }
        Member::DictionaryField { name, ty, doc, .. } => {
            let (port, descriptor) = map_type(ty);
            Some(Field {
                name: name.clone(),
                port,
                descriptor,
                offset: 0,
                size: 0,
                ptr_depth: 0,
                is_const: false,
                doc: doc.clone(),
            })
        }
        _ => None,
    }
}

fn member_to_function(intf: &str, m: &Member) -> Option<Function> {
    let (op_name, return_ty, args, doc) = match m {
        Member::Operation {
            name,
            return_ty,
            args,
            doc,
        } => (name.clone(), return_ty, args, doc.clone()),
        Member::Constructor { args, doc } => ("constructor".into(), &Type::Void, args, doc.clone()),
        _ => return None,
    };
    let snake = to_snake(&op_name);
    let full_name = format!("{}_{}", to_snake(intf), snake);
    let port_name = format!("{}{}", to_pascal(intf), to_pascal(&to_snake(&op_name)));
    let c_decl = format!("{}.{}", intf, op_name);
    Some(build_function(
        &full_name, port_name, return_ty, args, c_decl, doc,
    ))
}

fn callback_to_function(
    name: &str,
    return_ty: &Type,
    args: &[parser::Argument],
    doc: Option<String>,
) -> Option<Function> {
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
        doc,
    ))
}

fn build_function(
    name: &str,
    port_name: String,
    return_ty: &Type,
    args: &[parser::Argument],
    c_decl: String,
    doc: Option<String>,
) -> Function {
    let mut port_args = Vec::new();
    let mut arg_descriptors = Vec::new();
    let mut variadic = false;
    for a in args {
        let (port, descriptor) = map_type(&a.ty);
        if a.variadic {
            variadic = true;
        }
        port_args.push(Arg {
            name: a.name.clone(),
            port,
        });
        arg_descriptors.push(descriptor);
    }
    let (return_port, ret_descriptor) = map_type(return_ty);
    Function {
        name: name.to_string(),
        port_name,
        port_args,
        arg_descriptors,
        return_port,
        ret_descriptor,
        out_struct: None,
        variadic,
        errno_guard: true,
        c_decl,
        doc,
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
