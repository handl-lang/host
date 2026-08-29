use std::path::Path;

/// `handl.config.json` configuration.
///
/// Currently only the `lib` array is supported:
///
/// ```json
/// { "lib": ["hello", "json"] }
/// ```
#[derive(Debug, Default, serde::Deserialize)]
pub struct Config {
    /// Names of libraries to load by default.
    #[serde(default)]
    pub lib: Vec<String>,
}

impl Config {
    /// Loads a `Config` from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let text = std::fs::read_to_string(path.as_ref())?;
        let config: Config = serde_json::from_str(&text)?;
        Ok(config)
    }

    /// Searches the current directory for `handl.config.json` or `.handl/config.json`.
    pub fn find() -> Option<std::path::PathBuf> {
        let cwd = std::env::current_dir().ok()?;
        let a = cwd.join("handl.config.json");
        if a.is_file() {
            return Some(a);
        }
        let b = cwd.join(".handl").join("config.json");
        if b.is_file() {
            return Some(b);
        }
        None
    }
}
