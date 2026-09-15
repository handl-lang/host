//! The `SexpCompiler` implementation.

use handl_plugin_compile::{CompileError, Compiler, CompilerConfig, CompilerInput, CompilerOutput};
use plugin_engine::{ArrSource, RewriteWitness, Side, Term};
use serde_json::Value;

use crate::config::SexpConfig;
use crate::output::SexpOutput;

/// Compiler that emits Lisp-style S-expressions for a `Term`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SexpCompiler;

impl Compiler for SexpCompiler {
    fn compile(
        &self,
        input: &CompilerInput,
        config: &dyn CompilerConfig,
    ) -> Result<Box<dyn CompilerOutput>, CompileError> {
        let _cfg = config
            .as_any()
            .downcast_ref::<SexpConfig>()
            .ok_or_else(|| CompileError::InvalidInput("expected SexpConfig".to_string()))?;

        let sexp = emit_sexp(&input.term);
        Ok(Box::new(SexpOutput::new(sexp)))
    }

    fn target(&self) -> &str {
        "sexp"
    }
}

fn emit_sexp(term: &Term) -> String {
    let mut out = String::new();
    emit(term, &mut out);
    out
}

fn emit(term: &Term, out: &mut String) {
    match term {
        Term::Arr { value } => {
            out.push_str("(arr ");
            out.push_str(&value_to_sexp(value));
            out.push(')');
        }
        Term::Const { value } => {
            out.push_str("(const ");
            out.push_str(&value_to_sexp(value));
            out.push(')');
        }
        Term::Op {
            name,
            args,
            source,
            lit,
        } => {
            out.push('(');
            out.push_str(name);
            if !matches!(source, ArrSource::Native) {
                out.push(' ');
                out.push_str(&source_to_sexp(source));
            }
            if let Some(l) = lit {
                out.push(' ');
                out.push_str(&value_to_sexp(l));
            }
            for arg in args {
                out.push(' ');
                emit(arg, out);
            }
            out.push(')');
        }
        Term::Seq { fst, snd } => {
            out.push_str("(seq ");
            emit(fst, out);
            out.push(' ');
            emit(snd, out);
            out.push(')');
        }
        Term::First { inner } => {
            out.push_str("(first ");
            emit(inner, out);
            out.push(')');
        }
        Term::Choice { l, r } => {
            out.push_str("(choice ");
            emit(l, out);
            out.push(' ');
            emit(r, out);
            out.push(')');
        }
        Term::Loop { body } => {
            out.push_str("(loop ");
            emit(body, out);
            out.push(')');
        }
        Term::App { callee, args } => {
            out.push_str("(app ");
            emit(callee, out);
            for a in args {
                out.push(' ');
                emit(a, out);
            }
            out.push(')');
        }
        Term::Iter {
            fuel,
            predicate,
            step,
            initial,
        } => {
            out.push_str("(iter ");
            emit(fuel, out);
            out.push(' ');
            emit(predicate, out);
            out.push(' ');
            emit(step, out);
            out.push(' ');
            emit(initial, out);
            out.push(')');
        }
        Term::Child { role } => {
            out.push_str("(child ");
            out.push_str(role);
            out.push(')');
        }
        Term::Handle { body, target, impl_ } => {
            out.push_str("(handle ");
            emit(body, out);
            out.push(' ');
            out.push_str(target);
            out.push(' ');
            emit(impl_, out);
            out.push(')');
        }
        Term::Rewrite {
            source,
            target,
            witness,
        } => {
            out.push_str("(rewrite ");
            emit(source, out);
            out.push(' ');
            emit(target, out);
            out.push(' ');
            out.push_str(&witness_to_sexp(witness));
            out.push(')');
        }
        Term::Path {
            endpoints: (a, b),
            witness,
        } => {
            out.push_str("(path ");
            emit(a, out);
            out.push(' ');
            emit(b, out);
            if let Some(w) = witness {
                out.push(' ');
                emit(w, out);
            }
            out.push(')');
        }
        Term::Equiv { a, b, path } => {
            out.push_str("(equiv ");
            emit(a, out);
            out.push(' ');
            emit(b, out);
            if let Some(p) = path {
                out.push(' ');
                emit(p, out);
            }
            out.push(')');
        }
        Term::CellId { arrow } => {
            out.push_str("(cell-id ");
            emit(arrow, out);
            out.push(')');
        }
        Term::Src { cell } => {
            out.push_str("(src ");
            emit(cell, out);
            out.push(')');
        }
        Term::Tgt { cell } => {
            out.push_str("(tgt ");
            emit(cell, out);
            out.push(')');
        }
        Term::CellCompV { first, second } => {
            out.push_str("(cell-comp-v ");
            emit(first, out);
            out.push(' ');
            emit(second, out);
            out.push(')');
        }
        Term::Whisker { side, arrow, cell } => {
            out.push_str("(whisker ");
            out.push_str(match side {
                Side::Left => "left",
                Side::Right => "right",
            });
            out.push(' ');
            emit(arrow, out);
            out.push(' ');
            emit(cell, out);
            out.push(')');
        }
        Term::RecordProj { record, field } => {
            out.push_str("(record-proj ");
            emit(record, out);
            out.push(' ');
            out.push_str(field);
            out.push(')');
        }
        Term::RecordWith { record, field, value } => {
            out.push_str("(record-with ");
            emit(record, out);
            out.push(' ');
            out.push_str(field);
            out.push(' ');
            emit(value, out);
            out.push(')');
        }
        Term::Tag { tag, value } => {
            out.push_str("(tag ");
            out.push_str(tag);
            out.push(' ');
            emit(value, out);
            out.push(')');
        }
        Term::Univalence { equiv } => {
            out.push_str("(univalence ");
            emit(equiv, out);
            out.push(')');
        }
        Term::Glue { family, path } => {
            out.push_str("(glue ");
            emit(family, out);
            out.push(' ');
            emit(path, out);
            out.push(')');
        }
        Term::Fibrant { base, family } => {
            out.push_str("(fibrant ");
            emit(base, out);
            out.push(' ');
            emit(family, out);
            out.push(')');
        }
        Term::JElim { path, motive, base } => {
            out.push_str("(j-elim ");
            emit(path, out);
            out.push(' ');
            emit(motive, out);
            out.push(' ');
            emit(base, out);
            out.push(')');
        }
        Term::Refine { sort, predicate } => {
            out.push_str("(refine ");
            emit(sort, out);
            out.push(' ');
            emit(predicate, out);
            out.push(')');
        }
        Term::Cobind { grade, body } => {
            out.push_str("(cobind ");
            emit(grade, out);
            out.push(' ');
            emit(body, out);
            out.push(')');
        }
    }
}

fn value_to_sexp(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn source_to_sexp(source: &ArrSource) -> String {
    match source {
        ArrSource::Native => "native".to_string(),
        ArrSource::Foreign(s) => format!("(foreign {s})"),
        ArrSource::Judgment => "judgment".to_string(),
        ArrSource::DynamicForeign => "dynamic".to_string(),
    }
}

fn witness_to_sexp(witness: &RewriteWitness) -> String {
    match witness {
        RewriteWitness::Named(n) => format!("(named {n})"),
        RewriteWitness::Compute => "compute".to_string(),
        RewriteWitness::HandlerHint => "handler-hint".to_string(),
        RewriteWitness::Cast => "cast".to_string(),
    }
}
