//! Error types for the `hnl run` task runner.

use std::path::PathBuf;

use thiserror::Error;

/// Errors raised by script resolution, scheduling, execution, and caching.
#[derive(Error, Debug)]
pub enum RunError {
    /// No `pkg.hnl` was found walking up from the start directory.
    #[error("no pkg.hnl found walking up from {}", .0.display())]
    NoPackage(PathBuf),

    /// The named script does not exist in the resolved `pkg.hnl`.
    #[error("unknown script `{name}`{available}", available = if .available.is_empty() { String::new() } else { format!(" — available: {}", .available.join(", ")) })]
    UnknownScript {
        /// The requested script name.
        name: String,
        /// Names that *are* declared (for the npm-style hint).
        available: Vec<String>,
    },

    /// A `dependsOn` cycle was detected at plan time.
    #[error("dependsOn cycle: {}", .0.join(" -> "))]
    Cycle(Vec<String>),

    /// A task depends on a script that does not exist.
    #[error("script `{task}` dependsOn unknown script `{dep}`")]
    UnknownDep {
        /// The task declaring the dependency.
        task: String,
        /// The missing dependency name.
        dep: String,
    },

    /// A command line contains shell metacharacters — scripts are argv,
    /// not shell, in v1.
    #[error(
        "script `{script}` command `{command}` contains shell metacharacters ({chars}) — scripts are argv, not shell; wrap it in a script file and call `bash x.sh`"
    )]
    Metachar {
        /// The script the command belongs to.
        script: String,
        /// The offending command line.
        command: String,
        /// The metacharacter set found, for the diagnostic.
        chars: String,
    },

    /// A command could not be spawned (missing binary, bad cwd, …).
    #[error("cannot run `{argv0}`: {source}")]
    Spawn {
        /// argv[0] as written in the script.
        argv0: String,
        /// The underlying OS error.
        #[source]
        source: std::io::Error,
    },

    /// A task's `cwd` does not resolve to a directory.
    #[error("script `{script}` cwd `{cwd}` does not exist (resolved against {})", .base.display())]
    BadCwd {
        /// The script declaring the cwd.
        script: String,
        /// The declared cwd value.
        cwd: String,
        /// The package root it resolved against.
        base: PathBuf,
    },

    /// `HANDL_RUN_DEPTH` reached the recursion cap.
    #[error(
        "HANDL_RUN_DEPTH cap ({max}) exceeded — script→script recursion via `hnl run <name>`; break the cycle or raise the cap deliberately"
    )]
    DepthExceeded {
        /// The cap that was hit.
        max: u32,
    },

    /// A glob pattern could not be expanded.
    #[error("bad glob `{pattern}`: {source}")]
    Glob {
        /// The offending pattern.
        pattern: String,
        /// The underlying error.
        #[source]
        source: glob::PatternError,
    },

    /// Filesystem failure while reading inputs or writing the cache.
    #[error("{0}")]
    Io(String),
}

impl RunError {
    /// Convenience constructor for [`RunError::Io`].
    pub fn io(context: impl Into<String>) -> Self {
        RunError::Io(context.into())
    }
}
