use handl_platform::PlatformError;
use std::io::Read;

/// Specification for a CLI argument.
#[derive(Debug, Clone, Default)]
pub struct ArgSpec {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub multiple: bool,
    pub allow_hyphen_values: bool,
}

impl ArgSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn multiple(mut self) -> Self {
        self.multiple = true;
        self
    }

    pub fn allow_hyphen_values(mut self) -> Self {
        self.allow_hyphen_values = true;
        self
    }
}

/// Specification for a CLI command.
#[derive(Debug, Clone, Default)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub args: Vec<ArgSpec>,
}

impl CommandSpec {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            ..Default::default()
        }
    }

    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    pub fn arg(mut self, arg: ArgSpec) -> Self {
        self.args.push(arg);
        self
    }
}

/// Reads a source string from the given file path, stdin (`-` or empty), or stdin.
pub fn read_source_or_stdin(file: Option<&str>) -> Result<String, PlatformError> {
    match file {
        Some(p) if p == "-" || p.is_empty() => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| PlatformError::Plugin(e.to_string()))?;
            Ok(buf)
        }
        Some(p) => std::fs::read_to_string(p).map_err(|e| PlatformError::Plugin(e.to_string())),
        None => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| PlatformError::Plugin(e.to_string()))?;
            Ok(buf)
        }
    }
}
