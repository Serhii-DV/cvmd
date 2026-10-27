use crate::{application, config::Config};
use anyhow::{Context, Result};
use std::{env, fs, io::ErrorKind, path::Path};

fn path_status(path: &Path, directory: bool) -> String {
    match fs::metadata(path) {
        Ok(metadata) => {
            let status = if directory {
                if metadata.is_dir() {
                    "exists"
                } else {
                    "not a directory"
                }
            } else if metadata.is_file() {
                "exists"
            } else {
                "not a file"
            };
            status.into()
        }
        Err(error) if error.kind() == ErrorKind::NotFound => "missing".into(),
        Err(error) => format!("unavailable: {error}"),
    }
}

fn print_path(label: &str, path: &Path, directory: bool) {
    println!(
        "{label:<15} {} [{}]",
        path.display(),
        path_status(path, directory)
    );
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn executable_at(path: &Path) -> bool {
    if executable_file(path) {
        return true;
    }
    #[cfg(windows)]
    if path.extension().is_none() {
        let extensions = env::var_os("PATHEXT").unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
        for extension in extensions
            .to_string_lossy()
            .split(';')
            .filter(|value| !value.is_empty())
        {
            let mut candidate = path.as_os_str().to_owned();
            candidate.push(extension);
            if executable_file(Path::new(&candidate)) {
                return true;
            }
        }
    }
    false
}

fn renderer_available(renderer: &str, working_directory: &Path) -> bool {
    if renderer.is_empty() {
        return false;
    }
    let path = Path::new(renderer);
    if path.is_absolute() || path.components().count() > 1 {
        return executable_at(&working_directory.join(path));
    }
    env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths)
            .any(|directory| executable_at(&working_directory.join(directory).join(path)))
    })
}

pub fn show(config_path: &Path, config_source: &str) -> Result<()> {
    print_path("Config:", config_path, false);
    println!("{:<15} {config_source}", "Config source:");
    // Preserve the selected path in the report even when loading fails. The
    // normal error handler then provides a diagnostic and a nonzero exit code.
    let config = Config::load(config_path)?;
    print_path("Workspace:", &config.root, true);
    print_path("Source CV:", &config.resolve(&config.source)?, false);
    print_path(
        "Renderer config:",
        &config.resolve(&config.pdf.config_file)?,
        false,
    );
    let (browser, browser_source) = config.effective_browser();
    println!(
        "{:<15} {} [{browser_source}]",
        "Browser:",
        browser.as_deref().unwrap_or("auto-detect")
    );
    let (editor, source) = config.effective_editor();
    println!("{:<15} {editor} [{source}]", "Editor:");
    let application = application::discover(&config)?;
    let current_directory = env::current_dir()?;
    println!(
        "{:<15} {} [{}]",
        "PDF renderer:",
        config.pdf.renderer,
        if renderer_available(
            &config.pdf.renderer,
            application.as_deref().unwrap_or(&current_directory),
        ) {
            "available"
        } else {
            "unavailable"
        }
    );

    match application {
        None => println!("{:<15} none", "Application:"),
        Some(path) => {
            println!("{:<15} {}", "Application:", path.display());
            if let Some(aliases) = application::aliases(&path) {
                println!("{:<15} {}", "Company alias:", aliases.company);
                println!("{:<15} {}", "Position alias:", aliases.position);
            }
            let identity_path = path.join("application.toml");
            let identity = (|| -> Result<application::Identity> {
                let text = fs::read_to_string(&identity_path)
                    .with_context(|| format!("Cannot read {}", identity_path.display()))?;
                toml::from_str(&text)
                    .with_context(|| format!("Invalid identity in {}", identity_path.display()))
            })();
            match identity {
                Ok(identity) => {
                    println!("{:<15} {}", "Company:", identity.company);
                    println!("{:<15} {}", "Position:", identity.position);
                }
                Err(error) => println!("{:<15} unavailable ({error:#})", "Identity:"),
            }
        }
    }
    Ok(())
}
