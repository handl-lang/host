//! Validation for transform rule sets (ADR-0094 "Production readiness").

use std::collections::{BTreeMap, BTreeSet};

use handl_language::{AstShapeDef, Fragment, Language, RewriteStep, TransformCondition, TransformRule};
use handl_plugin_ext_diag::{Diagnostic, Severity, Span};

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
                            rule_id, start, shape.map(|s| s.node_kind.as_str()).unwrap_or("?"), n
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
        RewriteStep::Apply { rule_id: target, index } => {
            if lang.transforms.get_by_id(target).is_none() {
                diags.push(Diagnostic::new(
                    RULE,
                    Severity::Error,
                    Span::default(),
                    format!("rule {} Apply references unknown rule id '{}'", rule_id, target),
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
                            rule_id, index, shape.map(|s| s.node_kind.as_str()).unwrap_or("?")
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
        TransformCondition::KindAt { index, role, kind: _ } => {
            if let Some(i) = index {
                if let Some(n) = shape_role_count(shape) {
                    if *i >= n {
                        diags.push(Diagnostic::new(
                            RULE,
                            Severity::Error,
                            Span::default(),
                            format!(
                                "rule {} KindAt index {} is out of bounds for shape '{}'",
                                rule_id, i, shape.map(|s| s.node_kind.as_str()).unwrap_or("?")
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
                            format!("rule {} KindAt uses unknown role '{}' for shape '{}'", rule_id, r, s.node_kind),
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
        TransformCondition::PayloadEq { path: _, value: _ } => {
            // Payload paths are engine-defined; the abstraction crate cannot
            // validate them statically.
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
