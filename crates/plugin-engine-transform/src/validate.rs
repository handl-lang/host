//! Validation for transform rule sets (ADR-0094 "Production readiness").

use std::collections::{BTreeMap, BTreeSet};

use handl_language::{
    AstShapeDef, Fragment, Language, LspMatch, LspRule, RewriteStep, TransformCondition,
    TransformRule, Value,
};
use handl_plugin_ext_diag::{Diagnostic, Severity, Span};
use serde_json::Map;

const RULE: &str = "transform.validate";

/// Validate all transform rules in `lang`.
///
/// Returns a vector of [`Diagnostic`]s. The abstraction crate cannot know which
/// natives a future engine will wire, so `Native` rules are reported as a
/// warning. Missing target/create kinds are warnings because shape fragments may
/// come from a different source. Duplicate and out-of-bounds issues are errors.
pub fn validate_transforms(lang: &Language) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    let mut pass_kinds: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    // Pre-compute the set of node kinds that have rules per pass.
    for def in lang.transforms.iter() {
        pass_kinds
            .entry(def.pass.clone())
            .or_default()
            .insert(def.node_kind.clone());
    }

    for def in lang.transforms.iter() {
        let key = def.identity_key();
        if let Some(existing) = seen.get(&key) {
            diags.push(Diagnostic::new(
                RULE,
                Severity::Error,
                Span::default(),
                format!(
                    "duplicate rule for {} (existing id: {}, incoming id: {})",
                    key, existing, def.id
                ),
            ));
        } else {
            seen.insert(key, def.id.as_str());
        }

        let shape = lang.ast.get_by_identity(&def.node_kind);

        match &def.rule {
            TransformRule::Declarative {
                target_kind,
                children,
                payload: _,
            } => {
                if lang.ast.get_by_identity(target_kind).is_none() {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Warning,
                        Span::default(),
                        format!(
                            "rule {} targets unknown kind '{}' (declared shapes may come from another fragment)",
                            def.id, target_kind
                        ),
                    ));
                }

                if let Some(kinds) = pass_kinds.get(&def.pass) {
                    if kinds.contains(target_kind) && target_kind != &def.node_kind {
                        // Self-match is still a cycle but too noisy to warn twice.
                        diags.push(Diagnostic::new(
                            RULE,
                            Severity::Warning,
                            Span::default(),
                            format!(
                                "rule {} has target kind '{}' which matches another rule in pass '{}'; potential cycle under Fixpoint",
                                def.id, target_kind, def.pass
                            ),
                        ));
                    }
                }

                for step in children {
                    validate_step(step, lang, shape, &def.id, &mut diags);
                }
            }
            TransformRule::Native(name) => {
                diags.push(Diagnostic::new(
                    RULE,
                    Severity::Warning,
                    Span::default(),
                    format!(
                        "rule {} uses native rewrite '{}'; ensure it is registered in the wired NativeRewrite registry",
                        def.id, name
                    ),
                ));
            }
            TransformRule::Replace { path } => {
                // ADR-0127. `path` starts at the matched node, so its first
                // index is checked against the matched shape's roles —
                // exactly as `KeepPath` is (later steps cannot be statically
                // resolved to a child shape here).
                if path.is_empty() {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Error,
                        Span::default(),
                        format!(
                            "rule {} Replace path is empty; a non-empty child-index path is required",
                            def.id
                        ),
                    ));
                } else if let Some(n) = shape_role_count(shape) {
                    if path[0] >= n {
                        diags.push(Diagnostic::new(
                            RULE,
                            Severity::Error,
                            Span::default(),
                            format!(
                                "rule {} Replace first index {} is out of bounds for shape '{}' ({} declared roles)",
                                def.id,
                                path[0],
                                shape.map(|s| s.node_kind.as_str()).unwrap_or("?"),
                                n
                            ),
                        ));
                    }
                }
            }
        }

        if let Some(cond) = &def.when {
            validate_condition(cond, lang, shape, &def.id, &mut diags);
        }
    }

    diags
}

fn shape_role_count(shape: Option<&AstShapeDef>) -> Option<usize> {
    shape.and_then(|s| s.children.as_ref().map(|c| c.len()))
}

fn validate_step(
    step: &RewriteStep,
    lang: &Language,
    shape: Option<&AstShapeDef>,
    rule_id: &str,
    diags: &mut Vec<Diagnostic>,
) {
    match step {
        RewriteStep::Keep(i) | RewriteStep::Splice(i) => {
            if let Some(n) = shape_role_count(shape) {
                if *i >= n {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Error,
                        Span::default(),
                        format!(
                            "rule {} step {:?} index {} is out of bounds for shape '{}' ({} declared roles)",
                            rule_id, step, i, shape.map(|s| s.node_kind.as_str()).unwrap_or("?"), n
                        ),
                    ));
                }
            }
        }
        RewriteStep::KeepRole(role) => {
            if let Some(s) = shape {
                if s.role_index(role).is_none() {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Error,
                        Span::default(),
                        format!(
                            "rule {} uses unknown role '{}' for shape '{}'",
                            rule_id, role, s.node_kind
                        ),
                    ));
                }
            }
        }
        RewriteStep::KeepPath(path) => {
            if let Some(first) = path.first() {
                if let Some(n) = shape_role_count(shape) {
                    if *first >= n {
                        diags.push(Diagnostic::new(
                            RULE,
                            Severity::Error,
                            Span::default(),
                            format!(
                                "rule {} KeepPath first index {} is out of bounds for shape '{}' ({} declared roles)",
                                rule_id, first, shape.map(|s| s.node_kind.as_str()).unwrap_or("?"), n
                            ),
                        ));
                    }
                }
            }
            // Later path steps cannot be statically resolved to a child shape
            // here, so we validate only the first step.
        }
        RewriteStep::KeepRest(start) => {
            if let Some(n) = shape_role_count(shape) {
                if *start > n {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Error,
                        Span::default(),
                        format!(
                            "rule {} KeepRest({}) starts past shape '{}' arity ({} declared roles)",
                            rule_id,
                            start,
                            shape.map(|s| s.node_kind.as_str()).unwrap_or("?"),
                            n
                        ),
                    ));
                }
            }
        }
        RewriteStep::Create {
            kind,
            payload: _,
            children,
        } => {
            if lang.ast.get_by_identity(kind).is_none() {
                diags.push(Diagnostic::new(
                    RULE,
                    Severity::Warning,
                    Span::default(),
                    format!(
                        "rule {} creates unknown kind '{}' (declared shapes may come from another fragment)",
                        rule_id, kind
                    ),
                ));
            }
            // `Create` children are interpreted against the original rule node,
            // not against the created node's kind.
            for child in children {
                validate_step(child, lang, shape, rule_id, diags);
            }
        }
        RewriteStep::Fresh(_) => {}
        RewriteStep::Apply {
            rule_id: target,
            index,
        } => {
            if lang.transforms.get_by_id(target).is_none() {
                diags.push(Diagnostic::new(
                    RULE,
                    Severity::Error,
                    Span::default(),
                    format!(
                        "rule {} Apply references unknown rule id '{}'",
                        rule_id, target
                    ),
                ));
            }
            if let Some(n) = shape_role_count(shape) {
                if *index >= n {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Error,
                        Span::default(),
                        format!(
                            "rule {} Apply index {} is out of bounds for shape '{}'",
                            rule_id,
                            index,
                            shape.map(|s| s.node_kind.as_str()).unwrap_or("?")
                        ),
                    ));
                }
            }
        }
    }
}

fn validate_condition(
    cond: &TransformCondition,
    _lang: &Language,
    shape: Option<&AstShapeDef>,
    rule_id: &str,
    diags: &mut Vec<Diagnostic>,
) {
    match cond {
        TransformCondition::KindAt {
            index,
            role,
            kind: _,
        } => {
            if let Some(i) = index {
                if let Some(n) = shape_role_count(shape) {
                    if *i >= n {
                        diags.push(Diagnostic::new(
                            RULE,
                            Severity::Error,
                            Span::default(),
                            format!(
                                "rule {} KindAt index {} is out of bounds for shape '{}'",
                                rule_id,
                                i,
                                shape.map(|s| s.node_kind.as_str()).unwrap_or("?")
                            ),
                        ));
                    }
                }
            }
            if let Some(r) = role {
                if let Some(s) = shape {
                    if s.role_index(r).is_none() {
                        diags.push(Diagnostic::new(
                            RULE,
                            Severity::Error,
                            Span::default(),
                            format!(
                                "rule {} KindAt uses unknown role '{}' for shape '{}'",
                                rule_id, r, s.node_kind
                            ),
                        ));
                    }
                }
            }
            if let (Some(i), Some(r)) = (index, role) {
                if let Some(s) = shape {
                    if let Some(ri) = s.role_index(r) {
                        if ri != *i {
                            diags.push(Diagnostic::new(
                                RULE,
                                Severity::Error,
                                Span::default(),
                                format!(
                                    "rule {} KindAt index {} does not match role '{}' (resolves to index {})",
                                    rule_id, i, r, ri
                                ),
                            ));
                        }
                    }
                }
            }
        }
        TransformCondition::AnyChildKind { kind: _ } => {
            // Any child may satisfy the kind — nothing positional to check.
        }
        TransformCondition::PayloadEq { path: _, value: _ } => {
            // Payload paths are engine-defined; the abstraction crate cannot
            // validate them statically.
        }
        TransformCondition::ChildPayloadEq {
            index,
            role,
            path: _,
            value: _,
        } => {
            if let (Some(i), Some(s)) = (index, shape) {
                if let Some(n) = shape_role_count(Some(s)) {
                    if *i >= n {
                        diags.push(Diagnostic::new(
                            RULE,
                            Severity::Error,
                            Span::default(),
                            format!(
                                "rule {} ChildPayloadEq index {} is out of bounds for shape '{}'",
                                rule_id, i, s.node_kind
                            ),
                        ));
                    }
                }
            }
            if let (Some(r), Some(s)) = (role, shape) {
                if s.role_index(r).is_none() {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Error,
                        Span::default(),
                        format!(
                            "rule {} ChildPayloadEq uses unknown role '{}' for shape '{}'",
                            rule_id, r, s.node_kind
                        ),
                    ));
                }
            }
        }
        TransformCondition::ChildCount(n) => {
            if let Some(s) = shape {
                if let Some(roles) = &s.children {
                    let variadic = roles.iter().any(|r| r.variadic);
                    let required = roles.len();
                    if !variadic && *n < required {
                        diags.push(Diagnostic::new(
                            RULE,
                            Severity::Warning,
                            Span::default(),
                            format!(
                                "rule {} ChildCount({}) is below shape '{}' required role count ({})",
                                rule_id, n, s.node_kind, required
                            ),
                        ));
                    }
                }
            }
        }
        TransformCondition::And(conds) | TransformCondition::Or(conds) => {
            for c in conds {
                validate_condition(c, _lang, shape, rule_id, diags);
            }
        }
        TransformCondition::Not(c) => {
            validate_condition(c, _lang, shape, rule_id, diags);
        }
    }
}

// ---------------------------------------------------------------------------
// LSP surface validation (ADR-0100)
// ---------------------------------------------------------------------------

const KNOWN_LSP_KINDS: &[&str] = &[
    "export",
    "import_hover",
    "import_path",
    "import_name",
    "export_name",
    "symbol",
];

/// Validate all LSP surface definitions in `lang`.
///
/// Returns a vector of [`Diagnostic`]s. Error-severity diagnostics should fail
/// the `Language` assembly; warnings should not.
pub fn validate_lsp(lang: &Language) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut all_rule_ids = BTreeSet::new();

    for def in lang.lsp.iter() {
        for rule_id in def.rules.keys() {
            all_rule_ids.insert(rule_id.as_str());
        }
    }

    for def in lang.lsp.iter() {
        for (rule_id, rule) in &def.rules {
            validate_lsp_rule(rule, rule_id, &def.id, lang, &all_rule_ids, &mut diags);
        }

        for (method, rule_ids) in &def.lsp {
            for rule_id in rule_ids {
                if !all_rule_ids.contains(rule_id.as_str()) {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Error,
                        Span::default(),
                        format!(
                            "lsp method '{}' in fragment '{}' references unknown rule id '{}'",
                            method, def.id, rule_id
                        ),
                    ));
                }
            }
        }
    }

    diags
}

fn validate_lsp_rule(
    rule: &LspRule,
    rule_id: &str,
    def_id: &str,
    lang: &Language,
    all_rule_ids: &BTreeSet<&str>,
    diags: &mut Vec<Diagnostic>,
) {
    if !KNOWN_LSP_KINDS.contains(&rule.kind.as_str()) {
        diags.push(Diagnostic::new(
            RULE,
            Severity::Warning,
            Span::default(),
            format!(
                "rule '{}' in fragment '{}' has unknown kind '{}'",
                rule_id, def_id, rule.kind
            ),
        ));
    }

    let matched_shape = match &rule.match_condition {
        Some(LspMatch::Match { node_kind, when }) => {
            let shape = find_lsp_shape(lang, node_kind);
            if shape.is_none() {
                diags.push(Diagnostic::new(
                    RULE,
                    Severity::Error,
                    Span::default(),
                    format!(
                        "rule '{}' in fragment '{}' references unknown node kind '{}'",
                        rule_id, def_id, node_kind
                    ),
                ));
            }
            if let Some(cond) = when {
                validate_condition(cond, lang, shape, rule_id, diags);
            }
            shape
        }
        Some(LspMatch::Token { preceding, .. }) => {
            for tok in preceding {
                if lang.tokens.get_by_id(tok).is_none() {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Error,
                        Span::default(),
                        format!(
                            "rule '{}' in fragment '{}' references unknown token kind '{}' in preceding_tokens",
                            rule_id, def_id, tok
                        ),
                    ));
                }
            }
            None
        }
        Some(LspMatch::Native(_)) | None => None,
    };

    validate_lsp_rule_config(
        rule,
        rule_id,
        def_id,
        lang,
        all_rule_ids,
        matched_shape,
        diags,
    );
}

fn find_lsp_shape<'a>(lang: &'a Language, requested: &'a str) -> Option<&'a AstShapeDef> {
    if let Some(shape) = lang.ast.iter().find(|s| s.id == requested) {
        return Some(shape);
    }
    lang.ast.iter().find(|s| s.node_kind == requested)
}

fn validate_lsp_rule_config(
    rule: &LspRule,
    rule_id: &str,
    def_id: &str,
    lang: &Language,
    all_rule_ids: &BTreeSet<&str>,
    shape: Option<&AstShapeDef>,
    diags: &mut Vec<Diagnostic>,
) {
    validate_lsp_token_array(rule, rule_id, def_id, lang, "preceding_tokens", diags);
    validate_lsp_token_array(rule, rule_id, def_id, lang, "name_tokens", diags);

    if let Some(docs) = rule.config.get("docs").and_then(Value::as_object) {
        validate_lsp_trivia_list(docs, rule_id, def_id, lang, "leading_trivia", diags);
        validate_lsp_trivia_list(docs, rule_id, def_id, lang, "trailing_trivia", diags);
    }
    if let Some(docs) = rule.config.get("module_docs").and_then(Value::as_object) {
        validate_lsp_trivia_list(docs, rule_id, def_id, lang, "leading_trivia", diags);
        validate_lsp_trivia_list(docs, rule_id, def_id, lang, "trailing_trivia", diags);
    }

    if let Some(name_cfg) = rule.config.get("name").and_then(Value::as_object) {
        if let Some(role) = name_cfg.get("role").and_then(Value::as_str) {
            if let Some(s) = shape {
                if s.role_index(role).is_none() {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Error,
                        Span::default(),
                        format!(
                            "rule '{}' in fragment '{}' uses unknown role '{}' for shape '{}'",
                            rule_id, def_id, role, s.node_kind
                        ),
                    ));
                }
            }
        }
    }

    if let Some(sig_cfg) = rule.config.get("signature").and_then(Value::as_object) {
        if let Some(role) = sig_cfg.get("role").and_then(Value::as_str) {
            if let Some(s) = shape {
                if s.role_index(role).is_none() {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Error,
                        Span::default(),
                        format!(
                            "rule '{}' in fragment '{}' uses unknown role '{}' for shape '{}'",
                            rule_id, def_id, role, s.node_kind
                        ),
                    ));
                }
            }
        }
    }

    if let Some(sk) = rule.config.get("symbol_kind") {
        match sk {
            Value::String(s) => {
                if lsp_symbol_kind_number(s).is_none() {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Error,
                        Span::default(),
                        format!(
                            "rule '{}' in fragment '{}' has unknown symbol_kind '{}'",
                            rule_id, def_id, s
                        ),
                    ));
                }
            }
            Value::Number(_) => {
                if sk.as_u64().is_none() {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Error,
                        Span::default(),
                        format!(
                            "rule '{}' in fragment '{}' has invalid symbol_kind number",
                            rule_id, def_id
                        ),
                    ));
                }
            }
            _ => {
                diags.push(Diagnostic::new(
                    RULE,
                    Severity::Error,
                    Span::default(),
                    format!(
                        "rule '{}' in fragment '{}' has invalid symbol_kind (must be a SymbolKind name or number)",
                        rule_id, def_id
                    ),
                ));
            }
        }
    }

    if let Some(arr) = rule.config.get("export_rules").and_then(Value::as_array) {
        for v in arr {
            if let Some(id) = v.as_str() {
                if !all_rule_ids.contains(id) {
                    diags.push(Diagnostic::new(
                        RULE,
                        Severity::Error,
                        Span::default(),
                        format!(
                            "rule '{}' in fragment '{}' references unknown export rule '{}'",
                            rule_id, def_id, id
                        ),
                    ));
                }
            } else {
                diags.push(Diagnostic::new(
                    RULE,
                    Severity::Error,
                    Span::default(),
                    format!(
                        "rule '{}' in fragment '{}' has non-string export rule id",
                        rule_id, def_id
                    ),
                ));
            }
        }
    }
}

fn validate_lsp_token_array(
    rule: &LspRule,
    rule_id: &str,
    def_id: &str,
    lang: &Language,
    key: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(arr) = rule.config.get(key).and_then(Value::as_array) else {
        return;
    };
    for v in arr {
        if let Some(tok) = v.as_str() {
            if lang.tokens.get_by_id(tok).is_none() {
                diags.push(Diagnostic::new(
                    RULE,
                    Severity::Error,
                    Span::default(),
                    format!(
                        "rule '{}' in fragment '{}' references unknown token kind '{}' in '{}'",
                        rule_id, def_id, tok, key
                    ),
                ));
            }
        } else {
            diags.push(Diagnostic::new(
                RULE,
                Severity::Error,
                Span::default(),
                format!(
                    "rule '{}' in fragment '{}' has non-string token id in '{}'",
                    rule_id, def_id, key
                ),
            ));
        }
    }
}

fn validate_lsp_trivia_list(
    obj: &Map<String, Value>,
    rule_id: &str,
    def_id: &str,
    lang: &Language,
    key: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(arr) = obj.get(key).and_then(Value::as_array) else {
        return;
    };
    for v in arr {
        if let Some(tok) = v.as_str() {
            if lang.tokens.get_by_id(tok).is_none() {
                diags.push(Diagnostic::new(
                    RULE,
                    Severity::Error,
                    Span::default(),
                    format!(
                        "rule '{}' in fragment '{}' references unknown token kind '{}' in '{}'",
                        rule_id, def_id, tok, key
                    ),
                ));
            }
        } else {
            diags.push(Diagnostic::new(
                RULE,
                Severity::Error,
                Span::default(),
                format!(
                    "rule '{}' in fragment '{}' has non-string token id in '{}'",
                    rule_id, def_id, key
                ),
            ));
        }
    }
}

fn lsp_symbol_kind_number(s: &str) -> Option<u32> {
    let n = match s {
        "File" => 1,
        "Module" => 2,
        "Namespace" => 3,
        "Package" => 4,
        "Class" => 5,
        "Method" => 6,
        "Property" => 7,
        "Field" => 8,
        "Constructor" => 9,
        "Enum" => 10,
        "Interface" => 11,
        "Function" => 12,
        "Variable" => 13,
        "Constant" => 14,
        "String" => 15,
        "Number" => 16,
        "Boolean" => 17,
        "Array" => 18,
        "Object" => 19,
        "Key" => 20,
        "Null" => 21,
        "EnumMember" => 22,
        "Struct" => 23,
        "Event" => 24,
        "Operator" => 25,
        "TypeParameter" => 26,
        _ => s.parse::<u32>().ok()?,
    };
    Some(n)
}
