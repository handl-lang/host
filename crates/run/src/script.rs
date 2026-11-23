//! The script/task model: `pkg.hnl` `scripts:` values lowered into
//! runnable form.
//!
//! Three value forms (P-003):
//!
//! - **string** — one command line; runs every time, never cached;
//! - **array of strings** — sequential command lines, same semantics;
//! - **object** — a [`Task`]: `run` plus `dependsOn`/`cache`/`inputs`/
//!   `outputs`/`env`/`cwd`.
//!
//! Command lines are *argv, not shell*: each line splits on `&&` into
//! sequential segments, each segment splits on whitespace into argv.
//! There is no quote handling — an argument containing whitespace cannot
//! be expressed in v1 (wrap it in a script file and call `bash x.sh`).

use handl_platform::config::{PkgScript, PkgScriptTask, PkgScriptValue};

use crate::error::RunError;

/// One argv vector — a single executable invocation.
pub type Argv = Vec<String>;

/// A runnable script: either passthrough commands or a cacheable task.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    /// The script name as declared in `pkg.hnl`.
    pub name: String,
    /// The runnable body.
    pub body: ScriptBody,
}

/// What a script name resolves to.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptBody {
    /// String/array form — sequential command lines, never cached.
    Commands(Vec<String>),
    /// Object form — a task with scheduling and caching semantics.
    Task(Task),
}

/// The object form of a script — a runnable task.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Command lines (`run`), each possibly `&&`-chained.
    pub commands: Vec<String>,
    /// Names of sibling scripts that must run first.
    pub depends_on: Vec<String>,
    /// Whether this task participates in the cache (default `true`;
    /// `cache: false` opts out per-task).
    pub cache: bool,
    /// Declared `inputs` globs; `None` → the runner's default input set
    /// (module sources + `pkg.hnl`).
    pub inputs: Option<Vec<String>>,
    /// Declared `outputs` globs; `None`/empty → streams-only caching
    /// (no file restoration).
    pub outputs: Option<Vec<String>>,
    /// Environment variable names fingerprinted into the cache entry.
    pub env: Vec<String>,
    /// Working directory relative to the package root (`None` → root).
    pub cwd: Option<String>,
}

impl Script {
    /// Lower a parsed [`PkgScript`] into a [`Script`].
    pub fn from_pkg(script: &PkgScript) -> Self {
        let body = match &script.value {
            PkgScriptValue::Command(cmd) => ScriptBody::Commands(vec![cmd.clone()]),
            PkgScriptValue::Commands(cmds) => ScriptBody::Commands(cmds.clone()),
            PkgScriptValue::Task(task) => ScriptBody::Task(Task::from_pkg_task(task)),
        };
        Script {
            name: script.name.clone(),
            body,
        }
    }

    /// The command lines this script runs, in order.
    pub fn commands(&self) -> &[String] {
        match &self.body {
            ScriptBody::Commands(cmds) => cmds,
            ScriptBody::Task(task) => &task.commands,
        }
    }

    /// The task view, when this script is the object form.
    pub fn task(&self) -> Option<&Task> {
        match &self.body {
            ScriptBody::Task(task) => Some(task),
            ScriptBody::Commands(_) => None,
        }
    }

    /// Whether the script participates in caching at all: only object
    /// tasks with `cache != false` do.
    pub fn cacheable(&self, cache_enabled: bool) -> bool {
        cache_enabled && self.task().is_some_and(|t| t.cache)
    }

    /// Split the command lines into executable argv segments.
    ///
    /// Each line splits on `&&` into sequential segments; each segment
    /// splits on whitespace into argv. `extra_args` (the CLI args after
    /// the script name) append to the **last** segment — the npm `--`
    /// analogue.
    ///
    /// Fails with [`RunError::Metachar`] when a segment contains shell
    /// metacharacters — they are never interpreted.
    pub fn segments(&self, extra_args: &[String]) -> Result<Vec<Argv>, RunError> {
        let commands = self.commands();
        let mut segments: Vec<Argv> = Vec::new();
        for line in commands {
            for part in line.split("&&") {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                reject_metachars(&self.name, line, part)?;
                let mut argv: Argv = part.split_whitespace().map(str::to_string).collect();
                if argv.is_empty() {
                    continue;
                }
                segments.push(std::mem::take(&mut argv));
            }
        }
        if !extra_args.is_empty()
            && let Some(last) = segments.last_mut()
        {
            last.extend(extra_args.iter().cloned());
        }
        Ok(segments)
    }
}

impl Task {
    fn from_pkg_task(task: &PkgScriptTask) -> Self {
        Task {
            commands: task.run.clone(),
            depends_on: task.depends_on.clone(),
            cache: task.cache.unwrap_or(true),
            inputs: task.inputs.clone(),
            outputs: task.outputs.clone(),
            env: task.env.clone(),
            cwd: task.cwd.clone(),
        }
    }
}

/// Shell metacharacters rejected in script commands — argv, not shell.
/// `&` is included so a stray single `&` (background operator) fails;
/// the `&&` command separator is already split out before this check.
const METACHARS: &[char] = &['|', '>', '<', '$', '`', ';', '&', '(', ')', '\n', '\r'];

fn reject_metachars(script: &str, line: &str, segment: &str) -> Result<(), RunError> {
    let found: Vec<char> = METACHARS
        .iter()
        .copied()
        .filter(|c| segment.contains(*c))
        .collect();
    if found.is_empty() {
        return Ok(());
    }
    let chars = found
        .iter()
        .map(|c| match c {
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            c => c.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ");
    Err(RunError::Metachar {
        script: script.to_string(),
        command: line.to_string(),
        chars,
    })
}

/// A canonical, deterministic rendering of a task definition — the
/// "normalized script definition" half of the cache key. Field order is
/// fixed; lists keep declaration order (semantically irrelevant fields
/// like `depends_on` are included so any manifest edit busts the entry).
pub fn canonical_task(task: &Task) -> String {
    let join = |v: &[String]| v.join("\u{1f}");
    let opt = |v: &Option<Vec<String>>| v.as_ref().map_or("-".to_string(), |l| join(l));
    format!(
        "run={};deps={};cache={};inputs={};outputs={};env={};cwd={}",
        join(&task.commands),
        join(&task.depends_on),
        task.cache,
        opt(&task.inputs),
        opt(&task.outputs),
        join(&task.env),
        task.cwd.as_deref().unwrap_or("-"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::config::{PkgScriptTask, PkgScriptValue};

    fn script(body: ScriptBody) -> Script {
        Script {
            name: "s".into(),
            body,
        }
    }

    #[test]
    fn segments_split_on_and_and_whitespace() {
        let s = script(ScriptBody::Commands(vec![
            "echo one && echo two".into(),
            "cargo test -p foo".into(),
        ]));
        let segs = s.segments(&[]).unwrap();
        assert_eq!(
            segs,
            vec![
                vec!["echo".to_string(), "one".to_string()],
                vec!["echo".to_string(), "two".to_string()],
                vec![
                    "cargo".to_string(),
                    "test".to_string(),
                    "-p".to_string(),
                    "foo".to_string()
                ],
            ]
        );
    }

    #[test]
    fn extra_args_append_to_last_segment_only() {
        let s = script(ScriptBody::Commands(vec![
            "first cmd".into(),
            "second cmd && third cmd".into(),
        ]));
        let segs = s
            .segments(&["--filter".to_string(), "x".to_string()])
            .unwrap();
        assert_eq!(segs[0], vec!["first", "cmd"]);
        assert_eq!(segs[1], vec!["second", "cmd"]);
        assert_eq!(segs[2], vec!["third", "cmd", "--filter", "x"]);
    }

    #[test]
    fn metacharacters_are_rejected() {
        for bad in [
            "echo a | grep a",
            "echo a > f",
            "echo $HOME",
            "echo `id`",
            "echo a; echo b",
            "echo a & echo b",
            "echo (hi)",
        ] {
            let s = script(ScriptBody::Commands(vec![bad.to_string()]));
            let err = s.segments(&[]).unwrap_err();
            assert!(matches!(err, RunError::Metachar { .. }), "{bad}: {err}");
        }
        // `--` inside the command is fine (not a shell metachar).
        let s = script(ScriptBody::Commands(vec!["hnl run --json test".into()]));
        assert!(s.segments(&[]).is_ok());
    }

    #[test]
    fn task_defaults_and_cache_flag() {
        let t = PkgScriptTask {
            run: vec!["echo hi".into()],
            depends_on: vec!["dep".into()],
            ..PkgScriptTask::default()
        };
        let s = Script::from_pkg(&PkgScript {
            name: "t".into(),
            value: PkgScriptValue::Task(t),
        });
        let task = s.task().unwrap();
        assert!(task.cache, "tasks default to cached");
        assert!(s.cacheable(true));
        assert!(!s.cacheable(false), "--no-cache bypasses");

        let mut t2 = PkgScriptTask {
            run: vec!["echo hi".into()],
            cache: Some(false),
            ..PkgScriptTask::default()
        };
        let s2 = Script::from_pkg(&PkgScript {
            name: "t".into(),
            value: PkgScriptValue::Task(std::mem::take(&mut t2)),
        });
        assert!(!s2.cacheable(true), "cache: false opts out");

        let plain = script(ScriptBody::Commands(vec!["echo hi".into()]));
        assert!(!plain.cacheable(true), "scripts never cache");
    }

    #[test]
    fn canonical_task_is_deterministic() {
        let task = Task {
            commands: vec!["echo a".into()],
            depends_on: vec![],
            cache: true,
            inputs: Some(vec!["src/**".into()]),
            outputs: None,
            env: vec!["X".into()],
            cwd: None,
        };
        assert_eq!(canonical_task(&task), canonical_task(&task));
        let mut other = task.clone();
        other.commands = vec!["echo b".into()];
        assert_ne!(canonical_task(&task), canonical_task(&other));
    }
}
