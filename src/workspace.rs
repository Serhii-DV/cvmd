use crate::config::{self, Config};
use anyhow::{Context, Result, bail};
use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

fn create_if_missing(path: &Path, content: &[u8]) -> Result<bool> {
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            file.write_all(content)?;
            Ok(true)
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists && path.is_file() => Ok(false),
        Err(error) => Err(error).with_context(|| format!("Cannot create {}", path.display())),
    }
}

pub fn initialize(path: &Path, source: Option<&Path>, config_path: &Path) -> Result<PathBuf> {
    if config_path.try_exists()? {
        bail!(
            "Configuration already exists at {}; choose another workspace or use --config",
            config_path.display()
        );
    }
    let root = config::absolute(path, &std::env::current_dir()?)?;
    let source_path = root.join(config::DEFAULT_SOURCE);
    // Check before creating anything so an import never replaces an existing CV.
    if source.is_some() && source_path.try_exists()? {
        bail!("Source CV already exists at {}", source_path.display());
    }
    let source_content = match source {
        Some(path) => fs::read(config::expand_home(path)?)
            .with_context(|| format!("Cannot read source CV {}", path.display()))?,
        None => include_bytes!("../assets/cv.md").to_vec(),
    };
    for directory in ["assets", "applications"] {
        fs::create_dir_all(root.join(directory))
            .with_context(|| format!("Cannot create workspace directory {directory}"))?;
    }
    let created_source = create_if_missing(&source_path, &source_content)?;
    if source.is_some() && !created_source {
        bail!("Source CV already exists; import cancelled");
    }
    create_if_missing(
        &root.join("assets/cv.css"),
        include_bytes!("../assets/cv.css"),
    )?;
    create_if_missing(
        &root.join("assets/columns.css"),
        include_bytes!("../assets/columns.css"),
    )?;
    create_if_missing(
        &root.join(config::DEFAULT_PDF_CONFIG),
        include_bytes!("../assets/pdf.config.js"),
    )?;
    let root = root.canonicalize()?;
    let config_root = if config_path
        .parent()
        .and_then(|path| path.canonicalize().ok())
        .as_ref()
        == Some(&root)
    {
        PathBuf::from(".")
    } else {
        root.clone()
    };
    Config::new(config_root).save_new(config_path)?;
    Ok(root)
}
