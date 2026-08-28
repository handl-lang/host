//! Canonical expression host.
//!
//! `CanonicalHost` maps in-process canonical plugin ids to the concrete
//! parser contributions they provide, then builds a `CanonicalKit` from a
//! `KitManifest` using the `handl-platform` resolver for validation.

use std::collections::{HashMap, HashSet};

use handl_expr::{
    Diagnostics, ExpressionPipeline, Formatter, InfixFragment, ModulePipeline, PostfixFragment,
    PrimaryFragment, StatementFragment, TokenRule,
};
use handl_platform::{manifest::PluginManifest, resolve, PlatformError};
use handl_runtime::{RuntimePlugin, TypecheckPlugin};

use crate::kit_manifest::KitManifest;
use crate::CanonicalKit;

/// The concrete contributions of a single canonical in-process plugin.
#[derive(Default)]
pub struct PluginBundle {
    pub token_rule: Option<Box<dyn TokenRule>>,
    pub primary: Option<Box<dyn PrimaryFragment>>,
    pub postfix: Option<Box<dyn PostfixFragment>>,
    pub infix: Option<Box<dyn InfixFragment>>,
    pub statement: Option<Box<dyn StatementFragment>>,
    pub formatter: Option<Box<dyn Formatter>>,
    pub statement_formatter: Option<Box<handl_plugin_fmt_statements::StatementFormatter>>,
    pub diagnostics: Option<Box<dyn Diagnostics>>,
    pub typecheck: Option<Box<dyn TypecheckPlugin>>,
    pub runtime: Option<Box<dyn RuntimePlugin>>,
}

/// A host that knows how to instantiate every canonical in-process plugin.
pub struct CanonicalHost {
    factories: HashMap<String, Box<dyn Fn() -> PluginBundle + Send + Sync>>,
}

impl Default for CanonicalHost {
    fn default() -> Self {
        Self::register_builtin()
    }
}

impl CanonicalHost {
    /// Creates a host with all builtin canonical plugins registered.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads a `CanonicalKit` by resolving `manifest` with `handl-platform`
    /// and registering the plugin contributions in manifest order.
    pub fn load_kit(&self, manifest: &KitManifest) -> Result<CanonicalKit, PlatformError> {
        let mut pipeline = ExpressionPipeline::new();
        let mut token_count = 0usize;
        let mut primary_count = 0usize;
        let mut _postfix_count = 0usize;
        let mut infix_count = 0usize;
        let mut statements: Vec<Box<dyn StatementFragment>> = Vec::new();

        let mut plugin_manifests = Vec::new();

        let mut typecheck: Option<Box<dyn TypecheckPlugin>> = None;
        let mut runtime: Option<Box<dyn RuntimePlugin>> = None;

        for id in &manifest.plugins {
            if !self.factories.contains_key(id) {
                return Err(PlatformError::Plugin(format!(
                    "unknown canonical plugin: {id}"
                )));
            }

            let mut bundle = (self.factories[id])();

            if let Some(rule) = bundle.token_rule.take() {
                pipeline.register_token_rule(rule);
                token_count += 1;
            }
            if let Some(fragment) = bundle.primary.take() {
                pipeline.register_primary(fragment);
                primary_count += 1;
            }
            if let Some(fragment) = bundle.postfix.take() {
                pipeline.register_postfix(fragment);
                _postfix_count += 1;
            }
            if let Some(fragment) = bundle.infix.take() {
                pipeline.register_infix(fragment);
                infix_count += 1;
            }

            if let Some(fragment) = bundle.statement.take() {
                statements.push(fragment);
            }

            if let Some(plugin) = bundle.typecheck.take() {
                typecheck = Some(plugin);
            }
            if let Some(plugin) = bundle.runtime.take() {
                runtime = Some(plugin);
            }

            plugin_manifests.push(PluginManifest {
                id: id.clone(),
                version: manifest.version.clone(),
                provides: Vec::new(),
                requires: Vec::new(),
                optional: Vec::new(),
                extends: Vec::new(),
                conflicts: Vec::new(),
                replaces: Vec::new(),
                targets: Vec::new(),
                exclusive: Vec::new(),
                metadata: serde_json::Value::default(),
            });
        }

        // Validate the composition through the platform resolver.
        let resolved = resolve(&plugin_manifests, &[], None)
            .map_err(|e| PlatformError::Plugin(format!("kit resolution failed: {e}")))?;

        let selected: HashSet<_> = manifest.plugins.iter().cloned().collect();
        let resolved_set: HashSet<_> = resolved.selected_plugins.iter().cloned().collect();
        if selected != resolved_set {
            return Err(PlatformError::Plugin(
                "resolved plugin set does not match manifest".to_string(),
            ));
        }

        if token_count == 0 || primary_count == 0 || infix_count == 0 {
            return Err(PlatformError::Plugin(
                "kit must provide at least one token rule, primary, and infix fragment".to_string(),
            ));
        }

        let mut module = ModulePipeline::new(pipeline);
        for fragment in statements {
            module.register_statement(fragment);
        }

        Ok(CanonicalKit::from_module_with(module, typecheck, runtime))
    }

    fn register_builtin() -> Self {
        let mut factories: HashMap<String, Box<dyn Fn() -> PluginBundle + Send + Sync>> =
            HashMap::new();

        let mut add = |id: &'static str| {
            factories.insert(id.to_string(), Box::new(move || bundle_for(id).unwrap()));
        };

        // Token rules.
        add("plugin-token-operator");
        add("plugin-token-bytes");
        add("plugin-token-bool");
        add("plugin-token-null");
        add("plugin-token-keyword");
        add("plugin-token-string");
        add("plugin-token-number");
        add("plugin-token-comment");
        add("plugin-token-delimiter");
        add("plugin-token-identifier");

        // Primary fragments.
        add("primary-values");
        add("primary-identifiers");
        add("primary-arrays");
        add("primary-objects");
        add("primary-parens");
        add("primary-not");
        add("primary-typeof");
        add("primary-handle");
        add("primary-match");

        // Postfix fragments.
        add("postfix-calls");
        add("postfix-index");
        add("postfix-field");
        add("postfix-mood");
        add("function-literals");

        // Infix fragments.
        add("infix-flow");
        add("infix-fanout");
        add("infix-pipe");
        add("infix-ternary");
        add("infix-assign");

        // Statement fragments.
        add("stmt-let");
        add("stmt-const");
        add("stmt-import");
        add("stmt-export");
        add("stmt-module");

        // Formatter plugins.
        add("plugin-fmt-statements");

        // Non-parser expression plugins (empty bundles because the formatter
        // and diagnostics crates depend on this crate, so we can only keep
        // the slots ready here).
        add("typecheck-core");
        add("vm-rust");
        add("fmt-expression");
        add("diag-expression");

        Self { factories }
    }
}

fn bundle_for(id: &str) -> Option<PluginBundle> {
    let bundle = match id {
        "plugin-token-operator" => PluginBundle {
            token_rule: Some(Box::new(handl_plugin_operators::Operators::new())),
            ..Default::default()
        },
        "plugin-token-bytes" => PluginBundle {
            token_rule: Some(Box::new(handl_plugin_bytes::Bytes::new())),
            ..Default::default()
        },
        "plugin-token-bool" => PluginBundle {
            token_rule: Some(Box::new(handl_plugin_booleans::Booleans::new())),
            ..Default::default()
        },
        "plugin-token-null" => PluginBundle {
            token_rule: Some(Box::new(handl_plugin_null::Null::new())),
            ..Default::default()
        },
        "plugin-token-keyword" => PluginBundle {
            token_rule: Some(Box::new(handl_plugin_keywords::Keywords::new())),
            ..Default::default()
        },
        "plugin-token-string" => PluginBundle {
            token_rule: Some(Box::new(handl_plugin_strings::Strings::new())),
            ..Default::default()
        },
        "plugin-token-number" => PluginBundle {
            token_rule: Some(Box::new(handl_plugin_numbers::Numbers::new())),
            ..Default::default()
        },
        "plugin-token-comment" => PluginBundle {
            token_rule: Some(Box::new(handl_plugin_comment_tokens::CommentTokens::new())),
            ..Default::default()
        },
        "plugin-token-delimiter" => PluginBundle {
            token_rule: Some(Box::new(handl_plugin_delimiters::Delimiters::new())),
            ..Default::default()
        },
        "plugin-token-identifier" => PluginBundle {
            token_rule: Some(Box::new(handl_plugin_identifiers::Identifiers::new())),
            ..Default::default()
        },

        "primary-values" => PluginBundle {
            primary: Some(Box::new(handl_plugin_primary_values::PrimaryValues::new())),
            ..Default::default()
        },
        "primary-identifiers" => PluginBundle {
            primary: Some(Box::new(
                handl_plugin_primary_identifiers::PrimaryIdentifiers::new(),
            )),
            ..Default::default()
        },
        "primary-arrays" => PluginBundle {
            primary: Some(Box::new(handl_plugin_primary_arrays::PrimaryArrays::new())),
            ..Default::default()
        },
        "primary-objects" => PluginBundle {
            primary: Some(Box::new(handl_plugin_primary_objects::PrimaryObjects::new())),
            ..Default::default()
        },
        "primary-parens" => PluginBundle {
            primary: Some(Box::new(handl_plugin_primary_parens::PrimaryParens::new())),
            ..Default::default()
        },
        "primary-not" => PluginBundle {
            primary: Some(Box::new(handl_plugin_primary_not::PrimaryNot::new())),
            ..Default::default()
        },
        "primary-typeof" => PluginBundle {
            primary: Some(Box::new(
                handl_plugin_primary_typeof::PrimaryTypeOf::new(),
            )),
            ..Default::default()
        },
        "primary-handle" => PluginBundle {
            primary: Some(Box::new(handl_plugin_primary_handle::PrimaryHandle::new())),
            ..Default::default()
        },
        "primary-match" => PluginBundle {
            primary: Some(Box::new(handl_plugin_primary_match::PrimaryMatch::new())),
            ..Default::default()
        },

        "postfix-calls" => PluginBundle {
            postfix: Some(Box::new(handl_plugin_postfix_calls::PostfixCalls::new())),
            ..Default::default()
        },
        "postfix-index" => PluginBundle {
            postfix: Some(Box::new(handl_plugin_postfix_index::PostfixIndex::new())),
            ..Default::default()
        },
        "postfix-field" => PluginBundle {
            postfix: Some(Box::new(handl_plugin_postfix_field::PostfixField::new())),
            ..Default::default()
        },
        "postfix-mood" => PluginBundle {
            postfix: Some(Box::new(handl_plugin_postfix_mood::PostfixMood::new())),
            ..Default::default()
        },
        "function-literals" => PluginBundle {
            postfix: Some(Box::new(
                handl_plugin_function_literals::FunctionLiterals::new(),
            )),
            ..Default::default()
        },

        "infix-flow" => PluginBundle {
            infix: Some(Box::new(handl_plugin_infix_flow::Flow::new())),
            ..Default::default()
        },
        "infix-fanout" => PluginBundle {
            infix: Some(Box::new(handl_plugin_infix_fanout::Fanout::new())),
            ..Default::default()
        },
        "infix-pipe" => PluginBundle {
            infix: Some(Box::new(handl_plugin_infix_pipe::Pipe::new())),
            ..Default::default()
        },
        "infix-ternary" => PluginBundle {
            infix: Some(Box::new(handl_plugin_infix_ternary::Ternary::new())),
            ..Default::default()
        },
        "infix-assign" => PluginBundle {
            infix: Some(Box::new(handl_plugin_infix_assign::Assign::new())),
            ..Default::default()
        },

        // Statement fragments.
        "stmt-let" => PluginBundle {
            statement: Some(Box::new(handl_plugin_stmt_let::Let::new())),
            ..Default::default()
        },
        "stmt-const" => PluginBundle {
            statement: Some(Box::new(handl_plugin_stmt_const::Const::new())),
            ..Default::default()
        },
        "stmt-import" => PluginBundle {
            statement: Some(Box::new(handl_plugin_stmt_import::Import::new())),
            ..Default::default()
        },
        "stmt-export" => PluginBundle {
            statement: Some(Box::new(handl_plugin_stmt_export::Export::new())),
            ..Default::default()
        },
        "stmt-module" => PluginBundle {
            statement: Some(Box::new(handl_plugin_stmt_module::Module::new())),
            ..Default::default()
        },

        // Formatter plugins.
        "plugin-fmt-statements" => PluginBundle {
            statement_formatter: None,
            ..Default::default()
        },

        // Runtime and typecheck plugins.
        "typecheck-core" => PluginBundle {
            typecheck: Some(Box::new(handl_plugin_typecheck_core::TypecheckCore::new())),
            ..Default::default()
        },
        "vm-rust" => PluginBundle {
            runtime: Some(Box::new(handl_plugin_vm_rust::RustVm::new())),
            ..Default::default()
        },

        // Slots reserved for formatter/diagnostics plugins to avoid a
        // dependency cycle with this crate.
        "fmt-expression" | "diag-expression" => PluginBundle::default(),

        _ => return None,
    };

    Some(bundle)
}
