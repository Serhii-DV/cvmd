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

pub fn initialize(path: &Path, master: Option<&Path>, config_path: &Path) -> Result<PathBuf> {
    if config_path.try_exists()? {
        bail!(
            "Configuration already exists at {}; use --config for another workspace",
            config_path.display()
        );
    }
    let root = config::absolute(path, &std::env::current_dir()?)?;
    let master_content = match master {
        Some(path) => fs::read(config::expand_home(path)?)
            .with_context(|| format!("Cannot read master CV {}", path.display()))?,
        None => include_bytes!("../assets/cv.md").to_vec(),
    };
    // Check before creating anything so an import never replaces an existing CV.
    if master.is_some() && root.join("master/cv.md").try_exists()? {
        bail!(
            "Master CV already exists at {}",
            root.join("master/cv.md").display()
        );
    }
    for directory in ["master", "styles", "applications"] {
        fs::create_dir_all(root.join(directory))
            .with_context(|| format!("Cannot create workspace directory {directory}"))?;
    }
    let created_master = create_if_missing(&root.join("master/cv.md"), &master_content)?;
    if master.is_some() && !created_master {
        bail!("Master CV already exists; import cancelled");
    }
    create_if_missing(
        &root.join("styles/cv.css"),
        include_bytes!("../assets/cv.css"),
    )?;
    let root = root.canonicalize()?;
    Config::new(root.clone()).save_new(config_path)?;
    Ok(root)
}
