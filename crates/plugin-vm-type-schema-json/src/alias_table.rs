use handl_plugin_vm_type::Name;
use serde_json::Value;
use std::collections::HashMap;

/// A lookup table for local `$defs` entries.
#[derive(Debug, Default, Clone)]
pub struct AliasTable {
    entries: HashMap<Name, AliasEntry>,
}

#[derive(Debug, Clone)]
pub struct AliasEntry {
    pub type_params: Vec<Name>,
    pub schema: Value,
}

impl AliasTable {
    /// Build a table from the root document's `$defs` object.
    pub fn from_root(root: &Value) -> Self {
        let mut table = Self::default();
        if let Some(defs) = root.get("$defs").and_then(Value::as_object) {
            for (name, schema) in defs {
                let type_params = schema
                    .get("typeParams")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(Name::new))
                            .collect()
                    })
                    .unwrap_or_default();
                table.entries.insert(
                    Name::new(name),
                    AliasEntry {
                        type_params,
                        schema: schema.clone(),
                    },
                );
            }
        }
        table
    }

    pub fn lookup(&self, name: &Name) -> Option<&AliasEntry> {
        self.entries.get(name)
    }
}
