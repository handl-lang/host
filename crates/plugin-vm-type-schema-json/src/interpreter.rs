use crate::alias_table::AliasTable;
use crate::error::SchemaError;
use handl_plugin_vm_type::{
    Kind, Name, TCtx, Term, bvar, meta, named_ty, pi, sigma, tvar, type_app, type_lam, u0,
};
use serde_json::Value;
use std::collections::HashMap;

const MAX_DEPTH: usize = 64;

/// Stateful interpreter that converts a JSON Schema fragment into a `handl-plugin-vm-type` `Term`.
#[derive(Debug)]
pub struct SchemaInterpreter {
    table: AliasTable,
    stack: Vec<Name>,
    depth: usize,
    next_meta: u32,
    /// Type parameters introduced by `typeParams`, ordered by env index
    /// (most recently applied parameter first).
    type_param_names: Vec<Name>,
    /// Local names introduced by function parameters or record fields,
    /// ordered by env index (most recently introduced first).
    local_names: Vec<Name>,
    /// Memoised `$ref` resolutions keyed by the reference name and the
    /// current type/term-variable scope.
    ref_cache: HashMap<(Name, Vec<Name>, Vec<Name>), Term>,
}

impl SchemaInterpreter {
    pub fn new(root: &Value) -> Self {
        Self {
            table: AliasTable::from_root(root),
            stack: Vec::new(),
            depth: 0,
            next_meta: 0,
            type_param_names: Vec::new(),
            local_names: Vec::new(),
            ref_cache: HashMap::new(),
        }
    }

    /// Convert `schema` (resolved against `root`) into a `Term`.
    pub fn interpret(&mut self, schema: &Value) -> Result<Term, SchemaError> {
        self.interpret_with(schema)
    }

    fn interpret_with(&mut self, schema: &Value) -> Result<Term, SchemaError> {
        match schema {
            Value::Object(obj) => self.interpret_object(obj),
            _ => Ok(self.unknown()),
        }
    }

    fn interpret_object(
        &mut self,
        obj: &serde_json::Map<String, Value>,
    ) -> Result<Term, SchemaError> {
        let type_params: Vec<Name> = obj
            .get("typeParams")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(Name::new))
                    .collect()
            })
            .unwrap_or_default();

        for p in &type_params {
            self.type_param_names.insert(0, p.clone());
        }

        let core = self.interpret_core(obj);

        for _ in &type_params {
            self.type_param_names.remove(0);
        }

        let mut core = core?;
        for p in type_params.iter().rev() {
            core = type_lam(p.clone(), Kind::Type, core);
        }
        Ok(core)
    }

    fn interpret_core(
        &mut self,
        obj: &serde_json::Map<String, Value>,
    ) -> Result<Term, SchemaError> {
        if let Some(r) = obj.get("$ref") {
            return self.interpret_ref(obj, r);
        }

        if let Some(all_of) = obj.get("allOf").and_then(Value::as_array) {
            return self.interpret_combination(all_of);
        }
        if let Some(any_of) = obj.get("anyOf").and_then(Value::as_array) {
            return self.interpret_combination(any_of);
        }
        if let Some(one_of) = obj.get("oneOf").and_then(Value::as_array) {
            return self.interpret_combination(one_of);
        }
        if obj.get("not").is_some() {
            return Ok(self.unknown());
        }
        if obj.get("if").is_some() || obj.get("then").is_some() || obj.get("else").is_some() {
            let then = obj.get("then").map(|v| self.interpret_with(v));
            let else_ = obj.get("else").map(|v| self.interpret_with(v));
            return match (then, else_) {
                (Some(Ok(a)), Some(Ok(b))) if a == b => Ok(a),
                _ => Ok(self.unknown()),
            };
        }

        match obj.get("type").and_then(Value::as_str) {
            Some("string") => Ok(tvar("String")),
            Some("number") => Ok(tvar("Number")),
            Some("integer") => Ok(tvar("Int")),
            Some("boolean") => Ok(tvar("Bool")),
            Some("null") => Ok(tvar("Null")),
            Some("array") => self.interpret_array(obj),
            Some("object") => self.interpret_record(obj),
            Some("function") => self.interpret_function(obj),
            Some("var") => self.interpret_var(obj),
            Some(_) | None => Ok(self.unknown()),
        }
    }

    fn interpret_array(
        &mut self,
        obj: &serde_json::Map<String, Value>,
    ) -> Result<Term, SchemaError> {
        match obj.get("items") {
            Some(items) => {
                let inner = self.interpret_with(items)?;
                Ok(type_app(tvar("Array"), inner))
            }
            None => Ok(tvar("Array")),
        }
    }

    fn interpret_record(
        &mut self,
        obj: &serde_json::Map<String, Value>,
    ) -> Result<Term, SchemaError> {
        let properties = obj
            .get("properties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        let mut fields: Vec<(Name, Term)> = Vec::new();

        for (name, schema) in &properties {
            let fst = self.interpret_with(schema)?;
            self.local_names.insert(0, Name::new(name));
            fields.push((Name::new(name), fst));
        }

        if let Some(additional) = obj.get("additionalProperties").filter(|v| v.is_object()) {
            let fst = self.interpret_with(additional)?;
            self.local_names.insert(0, Name::new("__additional"));
            fields.push((Name::new("__additional"), fst));
        }

        if fields.is_empty() {
            return Ok(tvar("Object"));
        }

        for _ in &fields {
            self.local_names.remove(0);
        }

        let mut term = u0();
        for (name, fst) in fields.iter().rev() {
            term = sigma(name.clone(), fst.clone(), term);
        }
        Ok(term)
    }

    fn interpret_function(
        &mut self,
        obj: &serde_json::Map<String, Value>,
    ) -> Result<Term, SchemaError> {
        let params = obj
            .get("params")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut param_infos: Vec<(Name, Term)> = Vec::new();

        for param in &params {
            let param_obj = param.as_object().ok_or_else(|| {
                SchemaError::InvalidSchema("function param must be an object".to_string())
            })?;
            let name = param_obj
                .get("name")
                .and_then(Value::as_str)
                .map(Name::new)
                .unwrap_or_else(|| Name::new("arg"));
            let ty = param_obj
                .get("schema")
                .map(|v| self.interpret_with(v))
                .unwrap_or_else(|| Ok(self.unknown()))?;
            self.local_names.insert(0, name.clone());
            param_infos.push((name, ty));
        }

        let return_schema = obj.get("return").unwrap_or(&Value::Null);
        let mut body = self.interpret_with(return_schema)?;

        for _ in &params {
            self.local_names.remove(0);
        }

        for (name, ty) in param_infos.iter().rev() {
            body = pi(name.clone(), ty.clone(), body);
        }
        Ok(body)
    }

    fn interpret_var(&mut self, obj: &serde_json::Map<String, Value>) -> Result<Term, SchemaError> {
        let name = obj
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| SchemaError::InvalidSchema("var requires a name".to_string()))?;
        let name = Name::new(name);

        if let Some(i) = self.type_param_names.iter().position(|n| *n == name) {
            let idx = self.local_names.len() + i;
            return Ok(bvar(idx));
        }

        if self.local_names.contains(&name) {
            return Ok(tvar(name));
        }

        if self.table.lookup(&name).is_some() {
            return self.resolve_ref(&name, None);
        }

        Ok(tvar(name))
    }

    fn interpret_ref(
        &mut self,
        obj: &serde_json::Map<String, Value>,
        r: &Value,
    ) -> Result<Term, SchemaError> {
        let r = r
            .as_str()
            .ok_or_else(|| SchemaError::InvalidSchema("$ref must be a string".to_string()))?;
        let prefix = "#/$defs/";
        if !r.starts_with(prefix) {
            return Err(SchemaError::UnknownRef(r.to_string()));
        }
        let name = Name::new(&r[prefix.len()..]);
        let args = obj
            .get("args")
            .and_then(Value::as_array)
            .map(|v| v.as_slice());
        self.resolve_ref(&name, args)
    }

    fn resolve_ref(&mut self, name: &Name, args: Option<&[Value]>) -> Result<Term, SchemaError> {
        let key = (
            name.clone(),
            self.type_param_names.clone(),
            self.local_names.clone(),
        );
        if let Some(cached) = self.ref_cache.get(&key)
            && !self.stack.contains(name)
        {
            return Ok(cached.clone());
        }

        let entry = self
            .table
            .lookup(name)
            .ok_or_else(|| SchemaError::UnknownRef(name.to_string()))?
            .clone();

        if self.stack.contains(name) || self.depth >= MAX_DEPTH {
            return Ok(self.unknown());
        }

        self.stack.push(name.clone());
        self.depth += 1;
        let mut term = self.interpret_with(&entry.schema)?;
        self.depth -= 1;
        self.stack.pop();

        self.ref_cache.insert(key, term.clone());

        if let Some(args) = args {
            if entry.type_params.len() != args.len() {
                return Err(SchemaError::ArgCountMismatch {
                    name: name.to_string(),
                    expected: entry.type_params.len(),
                    found: args.len(),
                });
            }
            for arg in args {
                let a = self.interpret_with(arg)?;
                term = type_app(term, a);
            }
        }

        Ok(term)
    }

    fn interpret_combination(&mut self, arr: &[Value]) -> Result<Term, SchemaError> {
        let terms: Result<Vec<Term>, SchemaError> =
            arr.iter().map(|v| self.interpret_with(v)).collect();
        let terms = terms?;
        if terms.is_empty() {
            return Ok(self.unknown());
        }
        let first = terms[0].clone();
        if terms.iter().all(|t| *t == first) {
            Ok(first)
        } else {
            Ok(self.unknown())
        }
    }

    fn unknown(&mut self) -> Term {
        let id = self.next_meta;
        self.next_meta += 1;
        meta(id)
    }
}

/// A minimal typing context that binds the primitive types used by the
/// JSON Schema bridge (`String`, `Number`, `Int`, `Bool`, `Null`, `Object`,
/// `Unit`, and `Array`).
pub fn default_tctx() -> TCtx {
    let mut tctx = TCtx::new();
    for name in ["String", "Number", "Int", "Bool", "Null", "Object", "Unit"] {
        tctx = tctx.extend_with_kind(Name::new(name), named_ty(name), Kind::Type);
    }
    tctx = tctx.extend_with_kind(
        Name::new("Array"),
        named_ty("Array"),
        Kind::fun(Kind::Type, Kind::Type),
    );
    tctx
}

/// Convenience entry point for one-off interpretation.
pub fn interpret(schema: &Value, root: &Value) -> Result<Term, SchemaError> {
    SchemaInterpreter::new(root).interpret(schema)
}
