use crate::config::{self, Config};
use anyhow::{Context, Result, ensure};
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime};
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

pub struct Aliases {
    pub company: String,
    pub position: String,
}

// ASCII-only components work on Windows, macOS and Linux. The exact Unicode
// names live in application.toml, so alias generation is intentionally lossy.
fn alias(value: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !result.is_empty() {
                result.push('_');
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
    let now = Local::now();
    let parent = config
        .root
        .join("applications")
        .join(now.format("%Y/%m").to_string());
    fs::create_dir_all(&parent)?;
    let name = format!(
        "{}.{}.{}",
        now.format("%Y-%m-%d_%H-%M-%S"),
        alias(company),
        alias(position),
    );
    let path = reserve_directory(&parent, &name)?;
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

fn reserve_directory(parent: &Path, name: &str) -> Result<PathBuf> {
    let mut suffix = 1_u64;
    loop {
        let candidate = parent.join(if suffix == 1 {
            name.to_owned()
        } else {
            format!("{name}-{suffix}")
        });
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => suffix += 1,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Cannot create {}", candidate.display()));
            }
        }
    }
}

pub fn require_cv(path: PathBuf) -> Result<PathBuf> {
    ensure!(
        path.join("cv.md").is_file(),
        "Application CV is missing: {}",
        path.join("cv.md").display()
    );
    Ok(path)
}

// Both current-directory discovery and workspace search use the same layout
// validation. Date-only folders from earlier versions remain supported.
pub fn directory_date(path: &Path) -> Option<NaiveDate> {
    let name = path.file_name()?.to_str()?;
    let date = NaiveDate::parse_from_str(name.get(..10)?, "%Y-%m-%d").ok()?;
    let valid_name = if name.get(10..11)? == "-" {
        name.len() > 11
    } else {
        parse_aliases(name).is_some()
    };
    let month = path.parent()?;
    let year = month.parent()?;
    (valid_name
        && year.file_name()? == date.format("%Y").to_string().as_str()
        && month.file_name()? == format!("{:02}", date.month()).as_str())
    .then_some(date)
}

fn parse_aliases(name: &str) -> Option<Aliases> {
    let mut components = name.split('.');
    let timestamp = components.next()?;
    NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d_%H-%M-%S")
        .or_else(|_| NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d_%H-%M-%S-%f"))
        .ok()?;
    let company = components.next()?;
    let position = components.next()?;
    if components.next().is_some() {
        return None;
    }
    // Numeric collision suffixes are not part of the position alias.
    let position = position
        .rsplit_once('-')
        .map_or(position, |(alias, suffix)| {
            if !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()) {
                alias
            } else {
                position
            }
        });
    let valid = |value: &str| {
        !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    };
    if !valid(company) || !valid(position) {
        return None;
    }
    Some(Aliases {
        company: company.into(),
        position: position.into(),
    })
}

pub fn aliases(path: &Path) -> Option<Aliases> {
    let name = path.file_name()?.to_str()?;
    if let Some(aliases) = parse_aliases(name) {
        return Some(aliases);
    }
    // Older folders do not encode separate aliases; derive them from their
    // original names when identity metadata is available.
    let text = fs::read_to_string(path.join("application.toml")).ok()?;
    let identity: Identity = toml::from_str(&text).ok()?;
    Some(Aliases {
        company: alias(&identity.company),
        position: alias(&identity.position),
    })
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
        if relative.components().count() != 3 || directory_date(path).is_none() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_timestamp_collisions_preserve_files_and_searchable_aliases() {
        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().join("2026/10");
        fs::create_dir_all(&parent).unwrap();
        for name in [
            "2026-10-26_19-12-30.google.software_engineer",
            "2026-10-26_19-12-30-123456789.google.software_engineer",
        ] {
            let first = reserve_directory(&parent, name).unwrap();
            fs::write(first.join("cv.md"), "tailored CV").unwrap();
            let second = reserve_directory(&parent, name).unwrap();
            assert_eq!(second.file_name().unwrap(), format!("{name}-2").as_str());
            assert_eq!(
                fs::read_to_string(first.join("cv.md")).unwrap(),
                "tailored CV"
            );
            assert!(directory_date(&second).is_some());
            let aliases = aliases(&second).unwrap();
            assert_eq!(aliases.company, "google");
            assert_eq!(aliases.position, "software_engineer");
        }
    }
}
