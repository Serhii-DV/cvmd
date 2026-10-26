use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;

struct Workspace {
    temp: TempDir,
    root: PathBuf,
    config: PathBuf,
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn failure(output: Output, message: &str) {
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(message),
        "expected {message:?} in {stderr:?}"
    );
}

impl Workspace {
    fn empty() -> Self {
        let temp = tempfile::tempdir().unwrap();
        Self {
            root: temp.path().join("CV workspace with spaces"),
            config: temp.path().join("config/config.toml"),
            temp,
        }
    }

    fn new() -> Self {
        let workspace = Self::empty();
        success(
            workspace
                .command()
                .arg("init")
                .arg(&workspace.root)
                .arg("--master")
                .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cv.md"))
                .output()
                .unwrap(),
        );
        workspace
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cvmd"));
        command
            .arg("--config")
            .arg(&self.config)
            .current_dir(self.temp.path());
        command
    }

    fn application(&self, company: &str, position: &str) -> PathBuf {
        PathBuf::from(success(
            self.command()
                .args(["new", company, position])
                .output()
                .unwrap(),
        ))
    }

    fn configure(&self, section: &str, key: &str, value: &str) {
        let mut config: toml::Value =
            toml::from_str(&fs::read_to_string(&self.config).unwrap()).unwrap();
        if section.is_empty() {
            config[key] = value.into();
        } else {
            config[section][key] = value.into();
        }
        fs::write(&self.config, toml::to_string_pretty(&config).unwrap()).unwrap();
    }
}

#[test]
fn initializes_and_refuses_to_overwrite_configuration_or_master() {
    let workspace = Workspace::new();
    assert_eq!(
        fs::read(workspace.root.join("master/cv.md")).unwrap(),
        include_bytes!("fixtures/cv.md")
    );
    assert!(workspace.root.join("styles/cv.css").is_file());
    let original_config = fs::read(&workspace.config).unwrap();
    fs::write(workspace.root.join("master/cv.md"), "edited master").unwrap();
    failure(
        workspace
            .command()
            .arg("init")
            .arg(&workspace.root)
            .output()
            .unwrap(),
        "Configuration already exists",
    );
    assert_eq!(fs::read(&workspace.config).unwrap(), original_config);
    assert_eq!(
        fs::read_to_string(workspace.root.join("master/cv.md")).unwrap(),
        "edited master"
    );
}

#[test]
fn initializing_existing_workspace_preserves_cv_and_css() {
    let workspace = Workspace::empty();
    fs::create_dir_all(workspace.root.join("master")).unwrap();
    fs::create_dir_all(workspace.root.join("styles")).unwrap();
    fs::write(workspace.root.join("master/cv.md"), "my CV").unwrap();
    fs::write(workspace.root.join("styles/cv.css"), "my CSS").unwrap();
    success(
        workspace
            .command()
            .arg("init")
            .arg(&workspace.root)
            .output()
            .unwrap(),
    );
    assert_eq!(
        fs::read_to_string(workspace.root.join("master/cv.md")).unwrap(),
        "my CV"
    );
    assert_eq!(
        fs::read_to_string(workspace.root.join("styles/cv.css")).unwrap(),
        "my CSS"
    );
}

#[test]
fn missing_import_does_not_create_a_workspace() {
    let workspace = Workspace::empty();
    failure(
        workspace
            .command()
            .arg("init")
            .arg(&workspace.root)
            .args(["--master", "missing.md"])
            .output()
            .unwrap(),
        "Cannot read master CV",
    );
    assert!(!workspace.root.exists());
    assert!(!workspace.config.exists());
}

#[test]
fn preserves_exact_names_in_toml_and_copies_job_and_master() {
    let workspace = Workspace::new();
    let company = "ACME / R&D: \"北欧\" <A/S>?* | \\";
    let position = "Senior PHP/TS Developer: C++ & Rust™";
    let job = workspace.temp.path().join("job posting.md");
    fs::write(&job, "# Job\nA role description.").unwrap();
    let path = PathBuf::from(success(
        workspace
            .command()
            .args(["new", company, position])
            .arg("--job")
            .arg(&job)
            .output()
            .unwrap(),
    ));
    let identity: toml::Value =
        toml::from_str(&fs::read_to_string(path.join("application.toml")).unwrap()).unwrap();
    assert_eq!(identity["company"].as_str(), Some(company));
    assert_eq!(identity["position"].as_str(), Some(position));
    assert_eq!(identity.as_table().unwrap().len(), 2);
    let name = path.file_name().unwrap().to_str().unwrap();
    assert!(
        name.chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    );
    let date = chrono::NaiveDate::parse_from_str(&name[..10], "%Y-%m-%d").unwrap();
    assert_eq!(
        path.parent().unwrap(),
        workspace
            .root
            .join("applications")
            .join(date.format("%Y/%m").to_string())
    );
    assert_eq!(
        fs::read(path.join("job.md")).unwrap(),
        fs::read(job).unwrap()
    );
    assert_eq!(
        fs::read(path.join("cv.md")).unwrap(),
        include_bytes!("fixtures/cv.md")
    );
    assert_eq!(fs::read(path.join("notes.md")).unwrap(), b"");
    fs::write(path.join("cv.md"), "tailored CV").unwrap();
    assert_eq!(
        fs::read(workspace.root.join("master/cv.md")).unwrap(),
        include_bytes!("fixtures/cv.md")
    );
}

#[test]
fn collisions_and_lossy_slugs_never_overwrite_existing_applications() {
    let workspace = Workspace::new();
    let first = workspace.application("A/B", "Developer");
    fs::write(first.join("cv.md"), "tailored CV").unwrap();
    let second = workspace.application("A:B", "Developer");
    assert_eq!(
        second.file_name().unwrap().to_str().unwrap(),
        format!("{}-2", first.file_name().unwrap().to_str().unwrap())
    );
    assert_eq!(
        fs::read_to_string(first.join("cv.md")).unwrap(),
        "tailored CV"
    );
    let unicode = workspace.application("株式会社", "開発者");
    assert!(
        unicode
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("-unnamed-unnamed")
    );
    let long = workspace.application(&"A".repeat(500), &"B".repeat(500));
    assert!(long.file_name().unwrap().len() < 120);
}

#[test]
fn invalid_inputs_do_not_leave_partial_applications() {
    let workspace = Workspace::new();
    failure(
        workspace
            .command()
            .args(["new", " ", "Developer"])
            .output()
            .unwrap(),
        "Company cannot be empty",
    );
    failure(
        workspace
            .command()
            .args(["new", "ACME", "\t"])
            .output()
            .unwrap(),
        "Position cannot be empty",
    );
    failure(
        workspace
            .command()
            .args(["new", "ACME", "Developer", "--job", "missing.md"])
            .output()
            .unwrap(),
        "Cannot read job description",
    );
    fs::remove_file(workspace.root.join("master/cv.md")).unwrap();
    failure(
        workspace
            .command()
            .args(["new", "ACME", "Developer"])
            .output()
            .unwrap(),
        "Cannot read master CV",
    );
    assert_eq!(
        fs::read_dir(workspace.root.join("applications"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn reports_missing_and_malformed_configurations() {
    let workspace = Workspace::empty();
    failure(
        workspace
            .command()
            .args(["new", "ACME", "Developer"])
            .output()
            .unwrap(),
        "run `cvmd init",
    );
    fs::create_dir_all(workspace.config.parent().unwrap()).unwrap();
    fs::write(&workspace.config, "root = [").unwrap();
    failure(
        workspace.command().arg("build").output().unwrap(),
        "Invalid configuration",
    );
}

#[test]
fn relative_configuration_paths_are_resolved_against_the_config_file() {
    let workspace = Workspace::new();
    workspace.configure("", "root", "../CV workspace with spaces");
    assert!(workspace.application("ACME", "Developer").is_dir());
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn executable(workspace: &Workspace, name: &str, body: &str) -> PathBuf {
        let path = workspace.temp.path().join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn expands_home_in_workspace_master_and_stylesheet_paths() {
        let workspace = Workspace::new();
        workspace.configure("", "root", "~/CV workspace with spaces");
        workspace.configure("", "master", "~/CV workspace with spaces/master/cv.md");
        let path = PathBuf::from(success(
            workspace
                .command()
                .env("HOME", workspace.temp.path())
                .args(["new", "ACME", "Developer"])
                .output()
                .unwrap(),
        ));
        assert!(path.starts_with(&workspace.root));
        workspace.configure(
            "pdf",
            "stylesheet",
            "~/CV workspace with spaces/styles/cv.css",
        );
        let renderer = executable(
            &workspace,
            "renderer",
            "cat >/dev/null\nprintf '%s' '%PDF-1.7 mock'",
        );
        workspace.configure("pdf", "renderer", renderer.to_str().unwrap());
        success(
            workspace
                .command()
                .env("HOME", workspace.temp.path())
                .current_dir(path)
                .arg("build")
                .output()
                .unwrap(),
        );
    }

    #[test]
    fn editor_supports_arguments_nested_directories_and_filesystem_discovery() {
        let workspace = Workspace::new();
        let application = workspace.application("ACME", "Developer");
        let nested = application.join("attachments");
        fs::create_dir(&nested).unwrap();
        fs::remove_file(application.join("application.toml")).unwrap();
        let editor = executable(
            &workspace,
            "editor with spaces",
            "printf '%s\\n' \"$@\" > \"$CAPTURE\"",
        );
        let capture = workspace.temp.path().join("arguments.txt");
        workspace.configure("", "editor", "nonexistent-configured-editor");
        success(
            workspace
                .command()
                .current_dir(&nested)
                .env(
                    "EDITOR",
                    format!("'{}' --wait 'two words'", editor.display()),
                )
                .env("CAPTURE", &capture)
                .arg("edit")
                .output()
                .unwrap(),
        );
        assert_eq!(
            fs::read_to_string(&capture).unwrap(),
            format!(
                "--wait\ntwo words\n{}\n",
                application.join("cv.md").display()
            )
        );
        workspace.configure("", "editor", &format!("'{}'", editor.display()));
        success(
            workspace
                .command()
                .current_dir(&application)
                .env_remove("EDITOR")
                .env("CAPTURE", &capture)
                .arg("edit")
                .output()
                .unwrap(),
        );
        failure(
            workspace
                .command()
                .current_dir(&application)
                .env("EDITOR", "'unterminated")
                .arg("edit")
                .output()
                .unwrap(),
            "Invalid quoting",
        );
        let failing_editor = executable(&workspace, "failing-editor", "exit 9");
        failure(
            workspace
                .command()
                .current_dir(application)
                .env("EDITOR", failing_editor)
                .arg("edit")
                .output()
                .unwrap(),
            "Editor exited unsuccessfully",
        );
    }

    #[test]
    fn rejects_unrelated_directories_and_missing_application_cv() {
        let workspace = Workspace::new();
        failure(
            workspace.command().arg("build").output().unwrap(),
            "Run this command from a dated application directory",
        );
        let application = workspace.application("ACME", "Developer");
        fs::remove_file(application.join("cv.md")).unwrap();
        failure(
            workspace
                .command()
                .current_dir(application)
                .arg("edit")
                .output()
                .unwrap(),
            "Application CV is missing",
        );
    }

    #[test]
    fn renderer_receives_shared_styles_and_cv_and_failures_preserve_previous_pdf() {
        let workspace = Workspace::new();
        let application = workspace.application("ACME", "Developer");
        let renderer = executable(
            &workspace,
            "PDF renderer",
            "printf '%s\\n' \"$@\" > \"$ARGUMENTS\"\ncat > \"$INPUT\"\nprintf '%s' '%PDF-1.7 mock'",
        );
        let arguments = workspace.temp.path().join("renderer-args");
        let input = workspace.temp.path().join("renderer-input");
        workspace.configure("pdf", "renderer", renderer.to_str().unwrap());
        success(
            workspace
                .command()
                .current_dir(&application)
                .arg("build")
                .env("ARGUMENTS", &arguments)
                .env("INPUT", &input)
                .output()
                .unwrap(),
        );
        assert_eq!(
            fs::read_to_string(arguments).unwrap(),
            format!(
                "--stylesheet\n{}\n--basedir\n{}\n",
                workspace.root.join("styles/cv.css").display(),
                application.display()
            )
        );
        assert_eq!(fs::read(input).unwrap(), include_bytes!("fixtures/cv.md"));
        assert_eq!(
            fs::read(application.join("cv.pdf")).unwrap(),
            b"%PDF-1.7 mock"
        );
        let failing = executable(
            &workspace,
            "failing-renderer",
            "printf 'partial output'\nexit 7",
        );
        workspace.configure("pdf", "renderer", failing.to_str().unwrap());
        failure(
            workspace
                .command()
                .current_dir(&application)
                .arg("build")
                .output()
                .unwrap(),
            "PDF renderer exited unsuccessfully",
        );
        let invalid = executable(&workspace, "invalid-renderer", "printf 'not a PDF'");
        workspace.configure("pdf", "renderer", invalid.to_str().unwrap());
        failure(
            workspace
                .command()
                .current_dir(&application)
                .arg("build")
                .output()
                .unwrap(),
            "Renderer output is not a PDF",
        );
        workspace.configure("pdf", "renderer", "/missing/md-to-pdf");
        failure(
            workspace
                .command()
                .current_dir(&application)
                .arg("build")
                .output()
                .unwrap(),
            "Cannot start PDF renderer",
        );
        assert_eq!(
            fs::read(application.join("cv.pdf")).unwrap(),
            b"%PDF-1.7 mock"
        );
        assert_eq!(
            fs::read(application.join("cv.md")).unwrap(),
            include_bytes!("fixtures/cv.md")
        );
        assert_eq!(fs::read_dir(&application).unwrap().count(), 5);
        fs::remove_file(workspace.root.join("styles/cv.css")).unwrap();
        failure(
            workspace
                .command()
                .current_dir(application)
                .arg("build")
                .output()
                .unwrap(),
            "Cannot find shared stylesheet",
        );
    }
}

#[test]
fn info_reports_resolved_configuration_without_changing_workspace_files() {
    let workspace = Workspace::new();
    workspace.configure("pdf", "renderer", "/nonexistent/cvmd-test-renderer");
    let config_before = fs::read(&workspace.config).unwrap();
    let report = success(
        workspace
            .command()
            .env_remove("EDITOR")
            .arg("info")
            .output()
            .unwrap(),
    );
    for (label, value) in [
        ("Config:", workspace.config.display().to_string()),
        ("Config source:", "--config".into()),
        (
            "Workspace:",
            format!("{} [exists]", workspace.root.display()),
        ),
        (
            "Master CV:",
            format!("{} [exists]", workspace.root.join("master/cv.md").display()),
        ),
        (
            "Stylesheet:",
            format!(
                "{} [exists]",
                workspace.root.join("styles/cv.css").display()
            ),
        ),
        ("Editor:", "micro [config]".into()),
        (
            "PDF renderer:",
            "/nonexistent/cvmd-test-renderer [unavailable]".into(),
        ),
        ("Application:", "none".into()),
    ] {
        assert!(report.contains(&format!("{label:<15} {value}")), "{report}");
    }
    assert_eq!(fs::read(&workspace.config).unwrap(), config_before);
    assert_eq!(
        fs::read_dir(workspace.root.join("applications"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        fs::read(workspace.root.join("master/cv.md")).unwrap(),
        include_bytes!("fixtures/cv.md")
    );
}

#[test]
fn info_shows_selected_config_before_missing_or_invalid_config_errors() {
    let workspace = Workspace::empty();
    let output = workspace.command().arg("info").output().unwrap();
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(report.contains(workspace.config.to_str().unwrap()));
    assert!(report.contains("[missing]"));
    failure(output, "run `cvmd init <path>` first");
    assert!(!workspace.config.exists());
    assert!(!workspace.root.exists());
    fs::create_dir_all(workspace.config.parent().unwrap()).unwrap();
    fs::write(&workspace.config, "root = [").unwrap();
    let output = workspace.command().arg("info").output().unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains(workspace.config.to_str().unwrap()));
    failure(output, "Invalid configuration");
}

#[test]
fn info_reports_missing_files_and_workspace_without_failing() {
    let workspace = Workspace::new();
    workspace.configure("", "root", "../CV workspace with spaces");
    fs::remove_file(workspace.root.join("master/cv.md")).unwrap();
    fs::remove_file(workspace.root.join("styles/cv.css")).unwrap();
    let report = success(workspace.command().arg("info").output().unwrap());
    for relative in ["master/cv.md", "styles/cv.css"] {
        assert!(
            report.contains(&format!(
                "{} [missing]",
                workspace
                    .config
                    .parent()
                    .unwrap()
                    .join("../CV workspace with spaces")
                    .join(relative)
                    .display()
            )),
            "{report}"
        );
    }
    fs::remove_dir_all(&workspace.root).unwrap();
    success(workspace.command().arg("info").output().unwrap());
    fs::write(&workspace.root, "not a directory").unwrap();
    let report = success(workspace.command().arg("info").output().unwrap());
    assert!(report.contains("[not a directory]"));
}

#[test]
fn info_uses_the_effective_editor_and_identifies_its_source() {
    let workspace = Workspace::new();
    workspace.configure("", "editor", "configured-editor --wait");
    let report = success(
        workspace
            .command()
            .env("EDITOR", "environment-editor --wait")
            .arg("info")
            .output()
            .unwrap(),
    );
    assert!(report.contains("environment-editor --wait [$EDITOR]"));
    let report = success(
        workspace
            .command()
            .env("EDITOR", "  ")
            .arg("info")
            .output()
            .unwrap(),
    );
    assert!(report.contains("configured-editor --wait [config]"));
    let mut config: toml::Value =
        toml::from_str(&fs::read_to_string(&workspace.config).unwrap()).unwrap();
    config.as_table_mut().unwrap().remove("editor");
    fs::write(&workspace.config, toml::to_string_pretty(&config).unwrap()).unwrap();
    let report = success(
        workspace
            .command()
            .env_remove("EDITOR")
            .arg("info")
            .output()
            .unwrap(),
    );
    assert!(report.contains("micro [default]"));
}

#[test]
fn info_identifies_nested_applications_and_preserves_exact_names() {
    let workspace = Workspace::new();
    let company = "ACME / R&D: 北欧 A/S";
    let position = "Senior PHP/TS Developer™";
    let application = workspace.application(company, position);
    let nested = application.join("attachments");
    fs::create_dir(&nested).unwrap();
    // Diagnostics should still identify an application whose CV was removed.
    fs::remove_file(application.join("cv.md")).unwrap();
    let report = success(
        workspace
            .command()
            .current_dir(&nested)
            .arg("info")
            .output()
            .unwrap(),
    );
    assert!(report.contains(&format!("{:<15} {}", "Application:", application.display())));
    assert!(report.contains(&format!("{:<15} {company}", "Company:")));
    assert!(report.contains(&format!("{:<15} {position}", "Position:")));
    fs::remove_file(application.join("application.toml")).unwrap();
    let report = success(
        workspace
            .command()
            .current_dir(&nested)
            .arg("info")
            .output()
            .unwrap(),
    );
    assert!(report.contains("Identity:") && report.contains("Cannot read"));
    assert!(report.contains(application.to_str().unwrap()));
    fs::write(application.join("application.toml"), "company = [").unwrap();
    let report = success(
        workspace
            .command()
            .current_dir(nested)
            .arg("info")
            .output()
            .unwrap(),
    );
    assert!(report.contains("Invalid identity"));
}

#[cfg(target_os = "linux")]
#[test]
fn info_reports_default_config_selection() {
    let workspace = Workspace::empty();
    let config_directory = workspace.temp.path().join("user-config");
    let config = config_directory.join("cvmd/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        format!("root = {:?}\n", workspace.root.to_str().unwrap()),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cvmd"))
        .current_dir(workspace.temp.path())
        .env("XDG_CONFIG_HOME", &config_directory)
        .env_remove("EDITOR")
        .arg("info")
        .output()
        .unwrap();
    let report = success(output);
    assert!(report.contains(config.to_str().unwrap()));
    assert!(report.contains(&format!("{:<15} default", "Config source:")));
    assert!(report.contains("micro [default]"));
}

#[cfg(unix)]
#[test]
fn info_checks_renderer_executability_without_launching_it() {
    use std::os::unix::fs::PermissionsExt;
    let workspace = Workspace::new();
    let application = workspace.application("ACME", "Developer");
    let marker = application.join("executed");
    let renderer = application.join("test-renderer");
    fs::write(&renderer, "#!/bin/sh\ntouch executed\nexit 1\n").unwrap();
    fs::set_permissions(&renderer, fs::Permissions::from_mode(0o755)).unwrap();
    workspace.configure("pdf", "renderer", "test-renderer");
    let report = success(
        workspace
            .command()
            .env("PATH", &application)
            .arg("info")
            .output()
            .unwrap(),
    );
    assert!(report.contains("test-renderer [available]"));
    assert!(!marker.exists());
    workspace.configure("pdf", "renderer", renderer.to_str().unwrap());
    let report = success(workspace.command().arg("info").output().unwrap());
    assert!(report.contains(&format!("{} [available]", renderer.display())));
    let nested = application.join("attachments");
    fs::create_dir(&nested).unwrap();
    workspace.configure("pdf", "renderer", "./test-renderer");
    let report = success(
        workspace
            .command()
            .current_dir(&nested)
            .arg("info")
            .output()
            .unwrap(),
    );
    assert!(report.contains("./test-renderer [available]"));
    workspace.configure("pdf", "renderer", renderer.to_str().unwrap());
    fs::set_permissions(&renderer, fs::Permissions::from_mode(0o644)).unwrap();
    let report = success(workspace.command().arg("info").output().unwrap());
    assert!(report.contains(&format!("{} [unavailable]", renderer.display())));
    workspace.configure("pdf", "renderer", application.to_str().unwrap());
    let report = success(workspace.command().arg("info").output().unwrap());
    assert!(report.contains(&format!("{} [unavailable]", application.display())));
    assert!(!marker.exists());
}
