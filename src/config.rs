use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub root: PathBuf,
    #[serde(default = "default_source")]
    pub source: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    #[serde(default)]
    pub pdf: PdfConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PdfConfig {
    #[serde(default = "default_renderer")]
    pub renderer: String,
    #[serde(default = "default_stylesheet")]
    pub stylesheet: PathBuf,
}

pub const WORKSPACE_CONFIG: &str = "cvmd.toml";
pub const DEFAULT_SOURCE: &str = "assets/cv.md";
pub const DEFAULT_STYLESHEET: &str = "assets/cv.css";

pub struct Location {
    pub path: PathBuf,
    pub source: &'static str,
}

fn default_source() -> PathBuf {
    DEFAULT_SOURCE.into()
}
fn default_editor() -> String {
    "micro".into()
}
fn default_renderer() -> String {
    "md-to-pdf".into()
}
fn default_stylesheet() -> PathBuf {
    DEFAULT_STYLESHEET.into()
}

impl Default for PdfConfig {
    fn default() -> Self {
        Self {
            renderer: default_renderer(),
            stylesheet: default_stylesheet(),
        }
    }
}

pub fn expand_home(path: &Path) -> Result<PathBuf> {
    if path == Path::new("~") {
        return dirs::home_dir().context("Cannot determine home directory");
    }
    if let Ok(relative) = path.strip_prefix("~") {
        return Ok(dirs::home_dir()
            .context("Cannot determine home directory")?
            .join(relative));
    }
    Ok(path.to_owned())
}

pub fn absolute(path: &Path, base: &Path) -> Result<PathBuf> {
    let path = expand_home(path)?;
    Ok(if path.is_absolute() {
        path
    } else {
        base.join(path)
    })
}

pub fn initial_location(root: &Path, override_path: Option<&Path>) -> Result<PathBuf> {
    let current = std::env::current_dir()?;
    match override_path {
        Some(path) => absolute(path, &current),
        None => Ok(absolute(root, &current)?.join(WORKSPACE_CONFIG)),
    }
}

fn find_workspace_config(current: &Path) -> Result<Option<Location>> {
    for directory in current.ancestors() {
        let path = directory.join(WORKSPACE_CONFIG);
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                return Ok(Some(Location {
                    path,
                    source: "workspace",
                }));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => {
                return Err(error).with_context(|| format!("Cannot inspect {}", path.display()));
            }
        }
    }
    Ok(None)
}

pub fn local_location() -> Result<Location> {
    find_workspace_config(&std::env::current_dir()?)?.context(
        "No workspace configuration found in the current directory or its parents; run `cvmd init .` in the workspace root to create cvmd.toml",
    )
}

pub fn location(override_path: Option<&Path>) -> Result<Location> {
    let current = std::env::current_dir()?;
    if let Some(path) = override_path {
        return Ok(Location {
            path: absolute(path, &current)?,
            source: "--config",
        });
    }
    if let Some(location) = find_workspace_config(&current)? {
        return Ok(location);
    }
    // Keep existing installations working outside a workspace. These files are
    // selected as a whole, never merged with workspace configuration.
    Ok(Location {
        path: dirs::config_dir()
            .context("Cannot determine configuration directory; use --config")?
            .join("cvmd/config.toml"),
        source: "default",
    })
}

impl Config {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            source: default_source(),
            editor: Some(default_editor()),
            pdf: PdfConfig::default(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).with_context(|| {
            format!(
                "Cannot read {}; run `cvmd init <path>` first",
                path.display()
            )
        })?;
        Self::parse(&text, path)
    }

    pub fn parse(text: &str, path: &Path) -> Result<Self> {
        let mut config: Self = toml::from_str(text)
            .with_context(|| format!("Invalid configuration in {}", path.display()))?;
        if config.root.as_os_str().is_empty() {
            bail!("Workspace root cannot be empty");
        }
        config.root = absolute(
            &config.root,
            path.parent().context("Configuration has no parent")?,
        )?;
        Ok(config)
    }

    pub fn save_new(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self)?;
        fs::create_dir_all(path.parent().context("Configuration has no parent")?)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .with_context(|| {
                format!(
                    "Cannot create {}; existing configuration is never overwritten",
                    path.display()
                )
            })?;
        file.write_all(text.as_bytes())?;
        Ok(())
    }

    pub fn resolve(&self, path: &Path) -> Result<PathBuf> {
        absolute(path, &self.root)
    }

    pub fn effective_editor(&self) -> (String, &'static str) {
        if let Ok(editor) = std::env::var("EDITOR")
            && !editor.trim().is_empty()
        {
            return (editor, "$EDITOR");
        }
        match &self.editor {
            Some(editor) => (editor.clone(), "config"),
            None => (default_editor(), "default"),
        }
    }
}
