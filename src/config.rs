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
    #[serde(default = "default_master")]
    pub master: PathBuf,
    #[serde(default = "default_editor")]
    pub editor: String,
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

fn default_master() -> PathBuf {
    "master/cv.md".into()
}
fn default_editor() -> String {
    "micro".into()
}
fn default_renderer() -> String {
    "md-to-pdf".into()
}
fn default_stylesheet() -> PathBuf {
    "styles/cv.css".into()
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

pub fn location(override_path: Option<&Path>) -> Result<PathBuf> {
    let path = match override_path {
        Some(path) => path.to_owned(),
        None => dirs::config_dir()
            .context("Cannot determine configuration directory; use --config")?
            .join("cvmd/config.toml"),
    };
    absolute(&path, &std::env::current_dir()?)
}

impl Config {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            master: default_master(),
            editor: default_editor(),
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
        let mut config: Self = toml::from_str(&text)
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
}
