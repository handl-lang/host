//! The `DotCompiler` implementation.

use std::fmt::Write as _;

use handl_plugin_compile::{CompileError, Compiler, CompilerConfig, CompilerInput, CompilerOutput};
use plugin_engine::{ArrSource, RewriteWitness, Term};
use serde_json::Value;

use crate::config::DotConfig;
use crate::output::DotOutput;

/// Compiler that emits a Graphviz DOT digraph for a `Term` arrow graph.
#[derive(Debug, Default, Clone, Copy)]
pub struct DotCompiler;

impl Compiler for DotCompiler {
    fn compile(
        &self,
        input: &CompilerInput,
        config: &dyn CompilerConfig,
    ) -> Result<Box<dyn CompilerOutput>, CompileError> {
        let _cfg = config
            .as_any()
            .downcast_ref::<DotConfig>()
            .ok_or_else(|| CompileError::InvalidInput("expected DotConfig".to_string()))?;

        let dot = emit_dot(&input.term);
        Ok(Box::new(DotOutput::new(dot)))
    }

    fn target(&self) -> &str {
        "dot"
    }
}

struct Emitter<'a> {
    out: &'a mut String,
    next_id: usize,
}

impl<'a> Emitter<'a> {
    fn new(out: &'a mut String) -> Self {
        Self { out, next_id: 0 }
    }

    fn next(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn node(&mut self, label: &str) -> usize {
        let id = self.next();
        let escaped = dot_escape(label);
        writeln!(self.out, "  n{id}[label=\"{escaped}\"];").unwrap();
        id
    }

    fn edge(&mut self, from: usize, to: usize, label: &str) {
        let escaped = dot_escape(label);
        writeln!(self.out, "  n{from} -> n{to} [label=\"{escaped}\"];").unwrap();
    }
}

fn dot_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn emit_dot(term: &Term) -> String {
    let mut out = String::new();
    out.push_str("digraph {\n");
    out.push_str("  node [shape=box];\n");
    {
        let mut emitter = Emitter::new(&mut out);
        emit_term(term, &mut emitter);
    }
    out.push_str("}\n");
    out
}

fn emit_term(term: &Term, e: &mut Emitter) -> usize {
    match term {
        Term::Arr { value, .. } => {
            let label = format!("Arr {}", compact_value(value));
            e.node(&label)
        }
        Term::Const { value } => {
            let label = format!("Const {}", compact_value(value));
            e.node(&label)
        }
        Term::Op {
            name,
            args,
            source,
            lit,
            ..
        } => {
            let src = source_label(source);
            let lit_s = lit
                .as_ref()
                .map(|v| format!(" lit={}", compact_value(v)))
                .unwrap_or_default();
            let id = e.node(&format!("Op {name}{src}{lit_s}"));
            for (i, arg) in args.iter().enumerate() {
                let child = emit_term(arg, e);
                e.edge(id, child, &format!("arg[{i}]"));
            }
            id
        }
        Term::Seq { axis, fst, snd } => {
            // `>>>` is `>>>[0]`; draw the axis edge only for non-default
            // axes (ADR-0120 Phase 2).
            let label = match axis.nat_literal() {
                Some(0) => "Seq".to_string(),
                Some(k) => format!("Seq[{k}]"),
                None => "Seq[?]".to_string(),
            };
            let id = e.node(&label);
            if !axis.is_default_axis() {
                let ac = emit_term(axis, e);
                e.edge(id, ac, "axis");
            }
            let c1 = emit_term(fst, e);
            let c2 = emit_term(snd, e);
            e.edge(id, c1, "fst");
            e.edge(id, c2, "snd");
            id
        }
        Term::First { inner } => {
            let id = e.node("First");
            let c = emit_term(inner, e);
            e.edge(id, c, "inner");
            id
        }
        Term::Choice { l, r } => {
            let id = e.node("Choice");
            let c1 = emit_term(l, e);
            let c2 = emit_term(r, e);
            e.edge(id, c1, "l");
            e.edge(id, c2, "r");
            id
        }
        Term::Loop { body } => {
            let id = e.node("Loop");
            let c = emit_term(body, e);
            e.edge(id, c, "body");
            id
        }
        Term::App { callee, args } => {
            let id = e.node("App");
            let c = emit_term(callee, e);
            e.edge(id, c, "callee");
            for (i, arg) in args.iter().enumerate() {
                let child = emit_term(arg, e);
                e.edge(id, child, &format!("arg[{i}]"));
            }
            id
        }
        Term::Iter {
            fuel,
            predicate,
            step,
            initial,
        } => {
            let id = e.node("Iter");
            let c1 = emit_term(fuel, e);
            let c2 = emit_term(predicate, e);
            let c3 = emit_term(step, e);
            let c4 = emit_term(initial, e);
            e.edge(id, c1, "fuel");
            e.edge(id, c2, "predicate");
            e.edge(id, c3, "step");
            e.edge(id, c4, "initial");
            id
        }
        Term::Child { role } => {
            let label = format!("Child {role}");
            e.node(&label)
        }
        Term::Handle {
            body,
            target,
            impl_,
        } => {
            let id = e.node(&format!("Handle {target}"));
            let c1 = emit_term(body, e);
            let c2 = emit_term(impl_, e);
            e.edge(id, c1, "body");
            e.edge(id, c2, "impl");
            id
        }
        Term::Rewrite {
            source,
            target,
            witness,
        } => {
            let id = e.node(&format!("Rewrite {}", witness_label(witness)));
            let c1 = emit_term(source, e);
            let c2 = emit_term(target, e);
            e.edge(id, c1, "source");
            e.edge(id, c2, "target");
            id
        }
        Term::Src { cell } => {
            let id = e.node("Src");
            let c = emit_term(cell, e);
            e.edge(id, c, "cell");
            id
        }
        Term::Tgt { cell } => {
            let id = e.node("Tgt");
            let c = emit_term(cell, e);
            e.edge(id, c, "cell");
            id
        }
        Term::RecordProj { record, field } => {
            let id = e.node(&format!("RecordProj {field}"));
            let c = emit_term(record, e);
            e.edge(id, c, "record");
            id
        }
        Term::RecordWith {
            record,
            field,
            value,
        } => {
            let id = e.node(&format!("RecordWith {field}"));
            let c1 = emit_term(record, e);
            let c2 = emit_term(value, e);
            e.edge(id, c1, "record");
            e.edge(id, c2, "value");
            id
        }
        Term::Tag { tag, value } => {
            let id = e.node(&format!("Tag {tag}"));
            let c = emit_term(value, e);
            e.edge(id, c, "value");
            id
        }
        Term::Cobind { grade, body } => {
            let id = e.node("Cobind");
            let c1 = emit_term(grade, e);
            let c2 = emit_term(body, e);
            e.edge(id, c1, "grade");
            e.edge(id, c2, "body");
            id
        }
        Term::Opaque { data } => {
            let tag = data
                .get("__tag__")
                .and_then(|v| v.as_str())
                .unwrap_or("Opaque");
            e.node(&format!("Opaque {tag}"))
        }
    }
}

fn compact_value(value: &Value) -> String {
    let s = match serde_json::to_string(value) {
        Ok(s) => s,
        Err(_) => value.to_string(),
    };
    if s.len() > 40 {
        format!("{}...", &s[..40])
    } else {
        s
    }
}

fn source_label(source: &ArrSource) -> String {
    match source {
        ArrSource::Native => String::new(),
        ArrSource::Foreign(s) => format!(" [{s}]"),
        ArrSource::Judgment => " [judgment]".to_string(),
        ArrSource::DynamicForeign => " [dynamic]".to_string(),
    }
}

fn witness_label(witness: &RewriteWitness) -> String {
    match witness {
        RewriteWitness::Named(n) => format!("named={n}"),
        RewriteWitness::Compute => "compute".to_string(),
        RewriteWitness::HandlerHint => "handler-hint".to_string(),
        RewriteWitness::Cast => "cast".to_string(),
    }
}
