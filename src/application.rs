use crate::config::{self, Config};
use anyhow::{Context, Result, ensure};
use chrono::{Datelike, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Deserialize, Serialize)]
pub struct Identity {
    pub company: String,
    pub position: String,
}

// ASCII-only components work on Windows, macOS and Linux. The exact Unicode
// names live in application.toml, so slugging is intentionally lossy.
fn slug(value: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !result.is_empty() {
                result.push('-');
            }
            result.push(character.to_ascii_lowercase());
            separator = false;
            if result.len() >= 48 {
                break;
            }
        } else {
            separator = true;
        }
    }
    if result.is_empty() {
        "unnamed".into()
    } else {
        result
    }
}

pub fn create(
    config: &Config,
    company: &str,
    position: &str,
    job: Option<&Path>,
) -> Result<PathBuf> {
    ensure!(!company.trim().is_empty(), "Company cannot be empty");
    ensure!(!position.trim().is_empty(), "Position cannot be empty");
    // Read inputs before reserving the application directory.
    let master_path = config.resolve(&config.master)?;
    let cv = fs::read(&master_path)
        .with_context(|| format!("Cannot read master CV {}", master_path.display()))?;
    let job = match job {
        Some(path) => fs::read(config::expand_home(path)?)
            .with_context(|| format!("Cannot read job description {}", path.display()))?,
        None => Vec::new(),
    };
    let identity = toml::to_string_pretty(&Identity {
        company: company.to_owned(),
        position: position.to_owned(),
    })?;
    let date = Local::now().date_naive();
    let parent = config
        .root
        .join("applications")
        .join(date.format("%Y/%m").to_string());
    fs::create_dir_all(&parent)?;
    let name = format!("{date}-{}-{}", slug(company), slug(position));
    let mut suffix = 1_u64;
    let path = loop {
        let candidate = parent.join(if suffix == 1 {
            name.clone()
        } else {
            format!("{name}-{suffix}")
        });
        match fs::create_dir(&candidate) {
            Ok(()) => break candidate,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => suffix += 1,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Cannot create {}", candidate.display()));
            }
        }
    };
    let write_result = (|| -> Result<()> {
        fs::write(path.join("cv.md"), cv)?;
        fs::write(path.join("job.md"), job)?;
        fs::write(path.join("notes.md"), [])?;
        fs::write(path.join("application.toml"), identity)?;
        Ok(())
    })();
    if let Err(error) = write_result {
        // Only remove the directory just reserved by this invocation.
        let _ = fs::remove_dir_all(&path);
        return Err(error).context("Cannot write application files");
    }
    path.canonicalize()
        .context("Cannot resolve new application directory")
}

pub fn locate(config: &Config) -> Result<PathBuf> {
    let path = discover(config)?.context(
        "Run this command from a dated application directory (or one of its subdirectories)",
    )?;
    ensure!(
        path.join("cv.md").is_file(),
        "Application CV is missing: {}",
        path.join("cv.md").display()
    );
    Ok(path)
}

pub fn discover(config: &Config) -> Result<Option<PathBuf>> {
    let applications = match config.root.join("applications").canonicalize() {
        Ok(path) => path,
        Err(error) if matches!(error.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            return Ok(None);
        }
        Err(error) => return Err(error).context("Cannot resolve applications directory"),
    };
    let current = std::env::current_dir()?.canonicalize()?;
    for path in current.ancestors() {
        let Ok(relative) = path.strip_prefix(&applications) else {
            continue;
        };
        let parts: Vec<_> = relative.components().collect();
        if parts.len() != 3 {
            continue;
        }
        let name = parts[2].as_os_str().to_string_lossy();
        let Some(date_text) = name.get(..10) else {
            continue;
        };
        let Ok(date) = NaiveDate::parse_from_str(date_text, "%Y-%m-%d") else {
            continue;
        };
        if parts[0].as_os_str() != date.format("%Y").to_string().as_str()
            || parts[1].as_os_str() != format!("{:02}", date.month()).as_str()
            || !name[10..].starts_with('-')
        {
            continue;
        }
        return Ok(Some(path.to_owned()));
    }
    Ok(None)
}

pub fn edit(config: &Config, path: &Path) -> Result<()> {
    let (editor, _) = config.effective_editor();
    let arguments = shell_words::split(&editor).context("Invalid quoting in editor command")?;
    let (program, arguments) = arguments
        .split_first()
        .context("Editor command cannot be empty")?;
    let status = Command::new(program)
        .args(arguments)
        .arg(path.join("cv.md"))
        .current_dir(path)
        .status()
        .with_context(|| {
            format!("Cannot start editor {program}; set $EDITOR or editor in config.toml")
        })?;
    ensure!(status.success(), "Editor exited unsuccessfully: {status}");
    Ok(())
}
