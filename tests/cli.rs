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
                .arg("--source")
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

    fn named_application(&self, company: &str, position: &str, name: &str) -> PathBuf {
        let source = self.application(company, position);
        let path = self
            .root
            .join("applications")
            .join(&name[..4])
            .join(&name[5..7])
            .join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::rename(source, &path).unwrap();
        path
    }

    fn configure(&self, section: &str, key: &str, value: &str) {
        let mut config: toml::Value =
            toml::from_str(&fs::read_to_string(&self.config).unwrap()).unwrap();
        if section.is_empty() {
            config[key] = value.into();
        } else {
            config[section]
                .as_table_mut()
                .unwrap()
                .insert(key.to_owned(), value.into());
        }
        fs::write(&self.config, toml::to_string_pretty(&config).unwrap()).unwrap();
    }
}

#[path = "cli/configuration.rs"]
mod configuration;

#[path = "cli/pdf.rs"]
mod pdf;

#[test]
fn initializes_and_refuses_to_overwrite_configuration_or_source() {
    let workspace = Workspace::new();
    assert_eq!(
        fs::read(workspace.root.join("assets/cv.md")).unwrap(),
        include_bytes!("fixtures/cv.md")
    );
    assert!(workspace.root.join("assets/cv.css").is_file());
    let original_config = fs::read(&workspace.config).unwrap();
    fs::write(workspace.root.join("assets/cv.md"), "edited source").unwrap();
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
        fs::read_to_string(workspace.root.join("assets/cv.md")).unwrap(),
        "edited source"
    );
}

#[test]
fn initializing_existing_workspace_preserves_cv_and_css() {
    let workspace = Workspace::empty();
    fs::create_dir_all(workspace.root.join("assets")).unwrap();
    fs::write(workspace.root.join("assets/cv.md"), "my CV").unwrap();
    fs::write(workspace.root.join("assets/cv.css"), "my CSS").unwrap();
    success(
        workspace
            .command()
            .arg("init")
            .arg(&workspace.root)
            .output()
            .unwrap(),
    );
    assert_eq!(
        fs::read_to_string(workspace.root.join("assets/cv.md")).unwrap(),
        "my CV"
    );
    assert_eq!(
        fs::read_to_string(workspace.root.join("assets/cv.css")).unwrap(),
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
            .args(["--source", "missing.md"])
            .output()
            .unwrap(),
        "Cannot read source CV",
    );
    assert!(!workspace.root.exists());
    assert!(!workspace.config.exists());
}

#[test]
fn preserves_exact_names_in_toml_and_copies_job_and_source() {
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
            .all(|character| character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_' | '.'))
    );
    let components: Vec<_> = name.split('.').collect();
    assert_eq!(components.len(), 3);
    assert_eq!(components[0].len(), 19);
    assert!(chrono::NaiveDateTime::parse_from_str(components[0], "%Y-%m-%d_%H-%M-%S").is_ok());
    assert_eq!(components[1], "acme_r_d_a_s");
    assert_eq!(components[2], "senior_php_ts_developer_c_rust");
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
        fs::read(workspace.root.join("assets/cv.md")).unwrap(),
        include_bytes!("fixtures/cv.md")
    );
}

#[test]
fn collisions_and_lossy_slugs_never_overwrite_existing_applications() {
    let workspace = Workspace::new();
    let first = workspace.application("A/B", "Developer");
    fs::write(first.join("cv.md"), "tailored CV").unwrap();
    let second = workspace.application("A:B", "Developer");
    assert_ne!(first, second);
    assert!(
        first
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with(".a_b.developer")
    );
    let second_name = second.file_name().unwrap().to_str().unwrap();
    assert!(second_name.ends_with(".a_b.developer") || second_name.ends_with(".a_b.developer-2"));
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
            .ends_with(".unnamed.unnamed")
    );
    let long = workspace.application(&"A".repeat(500), &"B".repeat(500));
    assert!(long.file_name().unwrap().len() < 140);
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
    fs::remove_file(workspace.root.join("assets/cv.md")).unwrap();
    failure(
        workspace
            .command()
            .args(["new", "ACME", "Developer"])
            .output()
            .unwrap(),
        "Cannot read source CV",
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
        workspace
            .command()
            .args(["build", "acme"])
            .output()
            .unwrap(),
        "Invalid configuration",
    );
}

#[test]
fn bare_edit_and_build_show_help_with_missing_or_invalid_configuration() {
    let workspace = Workspace::empty();
    for invalid_config in [false, true] {
        if invalid_config {
            fs::create_dir_all(workspace.config.parent().unwrap()).unwrap();
            fs::write(&workspace.config, "root = [").unwrap();
        }
        for command in ["edit", "build"] {
            let expected = success(
                workspace
                    .command()
                    .args([command, "--help"])
                    .output()
                    .unwrap(),
            );
            let output = workspace.command().arg(command).output().unwrap();
            assert!(
                output.stderr.is_empty(),
                "help must not print configuration errors"
            );
            assert_eq!(success(output), expected);
            if command == "build" {
                assert_eq!(
                    success(
                        workspace
                            .command()
                            .args(["build", "--verbose"])
                            .output()
                            .unwrap()
                    ),
                    expected
                );
            }
            assert!(expected.contains(&format!("Usage: cvmd {command}")));
            assert!(expected.contains("company or position alias"));
            assert!(expected.contains(&format!("cvmd {command} google")));
        }
        assert!(!workspace.root.exists());
        if !invalid_config {
            assert!(!workspace.config.exists());
        }
    }
    assert_eq!(fs::read_to_string(workspace.config).unwrap(), "root = [");
}

#[test]
fn bare_edit_and_build_show_help_inside_and_outside_applications_without_running_tools() {
    let workspace = Workspace::new();
    let application = workspace.application("ACME", "Developer");
    let nested = application.join("attachments");
    fs::create_dir(&nested).unwrap();
    workspace.configure("pdf", "renderer", "/nonexistent/cvmd-test-renderer");
    let config_before = fs::read(&workspace.config).unwrap();
    for directory in [workspace.temp.path(), &application, &nested] {
        for command in ["edit", "build"] {
            let expected = success(
                workspace
                    .command()
                    .args([command, "--help"])
                    .output()
                    .unwrap(),
            );
            let output = workspace
                .command()
                .current_dir(directory)
                .env("EDITOR", "/nonexistent/cvmd-test-editor")
                .arg(command)
                .output()
                .unwrap();
            assert!(output.stderr.is_empty());
            assert_eq!(success(output), expected);
        }
    }
    assert!(!application.join("output/cv.pdf").exists());
    assert_eq!(fs::read(&workspace.config).unwrap(), config_before);
    assert_eq!(
        fs::read(application.join("cv.md")).unwrap(),
        include_bytes!("fixtures/cv.md")
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

    pub(super) fn executable(workspace: &Workspace, name: &str, body: &str) -> PathBuf {
        let path = workspace.temp.path().join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn expands_home_in_workspace_source_and_stylesheet_paths() {
        let workspace = Workspace::new();
        workspace.configure("", "root", "~/CV workspace with spaces");
        workspace.configure("", "source", "~/CV workspace with spaces/assets/cv.md");
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
            "~/CV workspace with spaces/assets/cv.css",
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
                .args(["build", "acme"])
                .output()
                .unwrap(),
        );
    }

    #[test]
    fn editor_supports_arguments_and_alias_search_from_nested_directories() {
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
                .args(["edit", "acme"])
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
                .args(["edit", "acme"])
                .output()
                .unwrap(),
        );
        failure(
            workspace
                .command()
                .current_dir(&application)
                .env("EDITOR", "'unterminated")
                .args(["edit", "acme"])
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
                .args(["edit", "acme"])
                .output()
                .unwrap(),
            "Editor exited unsuccessfully",
        );
    }

    #[test]
    fn rejects_missing_application_cv() {
        let workspace = Workspace::new();
        let application = workspace.application("ACME", "Developer");
        fs::remove_file(application.join("cv.md")).unwrap();
        failure(
            workspace
                .command()
                .current_dir(application)
                .args(["edit", "acme"])
                .output()
                .unwrap(),
            "Application CV is missing",
        );
    }

    #[test]
    fn renderer_receives_shared_styles_and_cv_and_failures_preserve_previous_pdf() {
        let workspace = Workspace::new();
        let application = workspace.application("ACME", "Developer");
        assert!(!application.join("output").exists());
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
                .args(["build", "acme"])
                .env("ARGUMENTS", &arguments)
                .env("INPUT", &input)
                .output()
                .unwrap(),
        );
        assert_eq!(
            fs::read_to_string(arguments).unwrap(),
            format!(
                "--stylesheet\n{}\n--basedir\n{}\n",
                workspace.root.join("assets/cv.css").display(),
                application.display()
            )
        );
        assert_eq!(fs::read(input).unwrap(), include_bytes!("fixtures/cv.md"));
        assert_eq!(
            fs::read(application.join("output/cv.pdf")).unwrap(),
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
                .args(["build", "acme"])
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
                .args(["build", "acme"])
                .output()
                .unwrap(),
            "Renderer output is not a PDF",
        );
        workspace.configure("pdf", "renderer", "/missing/md-to-pdf");
        failure(
            workspace
                .command()
                .current_dir(&application)
                .args(["build", "acme"])
                .output()
                .unwrap(),
            "Cannot start PDF renderer",
        );
        assert_eq!(
            fs::read(application.join("output/cv.pdf")).unwrap(),
            b"%PDF-1.7 mock"
        );
        assert_eq!(
            fs::read(application.join("cv.md")).unwrap(),
            include_bytes!("fixtures/cv.md")
        );
        assert_eq!(fs::read_dir(&application).unwrap().count(), 5);
        assert_eq!(fs::read_dir(application.join("output")).unwrap().count(), 1);
        fs::remove_file(workspace.root.join("assets/cv.css")).unwrap();
        failure(
            workspace
                .command()
                .current_dir(application)
                .args(["build", "acme"])
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
            "Source CV:",
            format!("{} [exists]", workspace.root.join("assets/cv.md").display()),
        ),
        (
            "Stylesheet:",
            format!(
                "{} [exists]",
                workspace.root.join("assets/cv.css").display()
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
        fs::read(workspace.root.join("assets/cv.md")).unwrap(),
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
    fs::remove_file(workspace.root.join("assets/cv.md")).unwrap();
    fs::remove_file(workspace.root.join("assets/cv.css")).unwrap();
    let report = success(workspace.command().arg("info").output().unwrap());
    for relative in ["assets/cv.md", "assets/cv.css"] {
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

#[test]
fn new_application_folders_use_second_precision_timestamps() {
    let workspace = Workspace::new();
    let first = workspace.application("Zulu", "Software Engineer");
    let second = workspace.application("Alpha", "Software Engineer");
    let first_name = first.file_name().unwrap().to_str().unwrap();
    let second_name = second.file_name().unwrap().to_str().unwrap();
    assert!(first_name.ends_with(".zulu.software_engineer"));
    assert!(second_name.ends_with(".alpha.software_engineer"));
    assert_eq!(first_name.split('.').next().unwrap().len(), 19);
    assert_eq!(second_name.split('.').next().unwrap().len(), 19);
    let first_time = chrono::NaiveDateTime::parse_from_str(
        first_name.split('.').next().unwrap(),
        "%Y-%m-%d_%H-%M-%S",
    )
    .unwrap();
    let second_time = chrono::NaiveDateTime::parse_from_str(
        second_name.split('.').next().unwrap(),
        "%Y-%m-%d_%H-%M-%S",
    )
    .unwrap();
    assert!(first_time <= second_time);
    if first_time < second_time {
        assert!(first < second);
    }
}

#[test]
fn info_and_current_directory_discovery_support_date_only_folders() {
    let workspace = Workspace::new();
    let application = workspace.named_application(
        "Legacy / Co",
        "Software Engineer",
        "2025-03-01-legacy-co-software-engineer",
    );
    let report = success(
        workspace
            .command()
            .current_dir(&application)
            .arg("info")
            .output()
            .unwrap(),
    );
    assert!(report.contains(application.to_str().unwrap()));
    assert!(report.contains(&format!("{:<15} legacy_co", "Company alias:")));
    assert!(report.contains(&format!("{:<15} software_engineer", "Position alias:")));
    fs::remove_file(application.join("application.toml")).unwrap();
    let report = success(
        workspace
            .command()
            .current_dir(&application)
            .arg("info")
            .output()
            .unwrap(),
    );
    assert!(report.contains(&format!("{:<15} {}", "Application:", application.display())));
}

#[test]
fn alias_search_rejects_empty_partial_and_missing_aliases() {
    let workspace = Workspace::new();
    let application = workspace.application("Google", "Software Engineer");
    for (query, error) in [
        ("", "Application alias cannot be empty"),
        ("goog", "No applications found"),
        ("missing", "No applications found"),
    ] {
        for command in ["edit", "build"] {
            failure(
                workspace.command().args([command, query]).output().unwrap(),
                error,
            );
        }
    }
    fs::remove_file(application.join("cv.md")).unwrap();
    failure(
        workspace
            .command()
            .args(["edit", "google"])
            .output()
            .unwrap(),
        "Application CV is missing",
    );
    failure(
        workspace
            .command()
            .args(["build", "software_engineer"])
            .output()
            .unwrap(),
        "Application CV is missing",
    );
}

#[cfg(unix)]
mod alias_selection {
    use super::*;
    use std::{io::Write, process::Stdio};

    fn with_input(mut command: Command, input: &str) -> Output {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    fn editor(workspace: &Workspace) -> PathBuf {
        unix::executable(
            workspace,
            "alias-editor",
            "printf '%s' \"$1\" > \"$CAPTURE\"",
        )
    }

    fn renderer(workspace: &Workspace) {
        let renderer = unix::executable(
            workspace,
            "alias-renderer",
            "cat >/dev/null\nprintf '%s' '%PDF-1.7 mock'",
        );
        workspace.configure("pdf", "renderer", renderer.to_str().unwrap());
    }

    #[test]
    fn single_company_or_position_match_works_without_metadata_from_any_directory() {
        let workspace = Workspace::new();
        let application = workspace.application("Google", "Software Engineer");
        let unrelated = workspace.application("Other", "Developer");
        fs::remove_file(application.join("application.toml")).unwrap();
        let capture = workspace.temp.path().join("opened-cv");
        let editor = editor(&workspace);
        let report = success(
            workspace
                .command()
                .current_dir(&application)
                .arg("info")
                .output()
                .unwrap(),
        );
        assert!(report.contains(&format!("{:<15} google", "Company alias:")));
        assert!(report.contains(&format!("{:<15} software_engineer", "Position alias:")));
        for query in ["GOOGLE", "software_engineer"] {
            success(
                workspace
                    .command()
                    .current_dir(&unrelated)
                    .args(["edit", query])
                    .env("EDITOR", &editor)
                    .env("CAPTURE", &capture)
                    .output()
                    .unwrap(),
            );
            assert_eq!(
                fs::read_to_string(&capture).unwrap(),
                application.join("cv.md").to_str().unwrap()
            );
        }
        renderer(&workspace);
        success(
            workspace
                .command()
                .args(["build", "software_engineer"])
                .output()
                .unwrap(),
        );
        assert_eq!(
            fs::read(application.join("output/cv.pdf")).unwrap(),
            b"%PDF-1.7 mock"
        );
        assert!(!unrelated.join("output/cv.pdf").exists());
    }

    #[test]
    fn multiple_matches_are_newest_first_and_invalid_numbers_prompt_again() {
        let workspace = Workspace::new();
        let oldest = workspace.named_application(
            "Google",
            "Developer",
            "2025-12-31_23-59-59-000000001.google.developer",
        );
        let newest = workspace.named_application(
            "Google",
            "Software Engineer",
            "2026-10-26_19-12-30.google.software_engineer",
        );
        let unrelated = workspace.application("Other", "Developer");
        let capture = workspace.temp.path().join("opened-cv");
        let mut command = workspace.command();
        command
            .args(["edit", "google"])
            .env("EDITOR", editor(&workspace))
            .env("CAPTURE", &capture);
        let output = with_input(command, "0\n99\ninvalid\n2\n");
        let diagnostics = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostics.contains("Found 2 applications (newest first)"));
        assert!(diagnostics.contains(&format!("1. {}", newest.display())));
        assert!(diagnostics.contains(&format!("2. {}", oldest.display())));
        assert!(diagnostics.contains("Enter a number from 1 to 2"));
        assert!(!diagnostics.contains(unrelated.to_str().unwrap()));
        assert!(
            success(output).is_empty(),
            "selection prompts must stay off stdout"
        );
        assert_eq!(
            fs::read_to_string(capture).unwrap(),
            oldest.join("cv.md").to_str().unwrap()
        );
    }

    #[test]
    fn build_selects_only_the_chosen_position_match_and_handles_collision_suffixes() {
        let workspace = Workspace::new();
        let oldest = workspace.named_application(
            "Google",
            "Software Engineer",
            "2026-10-26_19-12-30.google.software_engineer",
        );
        let newest = workspace.named_application(
            "Google",
            "Software Engineer",
            "2026-10-26_19-12-30.google.software_engineer-2",
        );
        fs::remove_file(newest.join("application.toml")).unwrap();
        renderer(&workspace);
        let mut command = workspace.command();
        command.args(["build", "software_engineer"]);
        let report = success(with_input(command, "1\n"));
        assert!(report.contains(newest.join("output/cv.pdf").to_str().unwrap()));
        assert_eq!(
            fs::read(newest.join("output/cv.pdf")).unwrap(),
            b"%PDF-1.7 mock"
        );
        assert!(!oldest.join("output/cv.pdf").exists());
    }

    #[test]
    fn cancellation_and_end_of_input_do_not_launch_tools() {
        let workspace = Workspace::new();
        let first = workspace.application("Google", "Developer");
        let second = workspace.application("Google", "Software Engineer");
        let capture = workspace.temp.path().join("opened-cv");
        renderer(&workspace);
        for command_name in ["edit", "build"] {
            for (input, message) in [
                ("q\n", "Application selection cancelled"),
                ("\n", "Application selection cancelled"),
                ("", "No selection received"),
            ] {
                let mut command = workspace.command();
                command
                    .args([command_name, "google"])
                    .env("EDITOR", editor(&workspace))
                    .env("CAPTURE", &capture);
                failure(with_input(command, input), message);
                assert!(!capture.exists());
                assert!(!first.join("output/cv.pdf").exists());
                assert!(!second.join("output/cv.pdf").exists());
            }
        }
    }

    #[test]
    fn legacy_folders_support_alias_selection_and_info_without_metadata() {
        let workspace = Workspace::new();
        let application = workspace.named_application(
            "Legacy / Co",
            "Software Engineer",
            "2025-03-01-legacy-co-software-engineer",
        );
        let capture = workspace.temp.path().join("opened-cv");
        let editor = editor(&workspace);
        success(
            workspace
                .command()
                .args(["edit", "legacy_co"])
                .env("EDITOR", &editor)
                .env("CAPTURE", &capture)
                .output()
                .unwrap(),
        );
        assert_eq!(
            fs::read_to_string(&capture).unwrap(),
            application.join("cv.md").to_str().unwrap()
        );
        renderer(&workspace);
        success(
            workspace
                .command()
                .args(["build", "software_engineer"])
                .output()
                .unwrap(),
        );
        fs::remove_file(application.join("application.toml")).unwrap();
        let nested = application.join("attachments");
        fs::create_dir(&nested).unwrap();
        let report = success(
            workspace
                .command()
                .current_dir(nested)
                .arg("info")
                .output()
                .unwrap(),
        );
        assert!(report.contains(application.to_str().unwrap()));
        assert_eq!(
            fs::read(application.join("output/cv.pdf")).unwrap(),
            b"%PDF-1.7 mock"
        );
    }

    #[test]
    fn search_skips_invalid_layouts_and_directory_symlinks() {
        use std::os::unix::fs::symlink;
        let workspace = Workspace::new();
        workspace.named_application(
            "Google",
            "Developer",
            "2026-10-26_99-99-99-000000001.google.developer",
        );
        let wrong_month = workspace.named_application(
            "Google",
            "Developer",
            "2026-11-26_19-12-30-123456789.google.developer",
        );
        let moved = workspace
            .root
            .join("applications/2026/10")
            .join(wrong_month.file_name().unwrap());
        fs::rename(wrong_month, moved).unwrap();
        let outside = workspace.temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("cv.md"), "external CV").unwrap();
        symlink(
            outside,
            workspace
                .root
                .join("applications/2026/10/2026-10-26_19-12-30-123456789.google.developer"),
        )
        .unwrap();
        failure(
            workspace
                .command()
                .args(["edit", "google"])
                .output()
                .unwrap(),
            "No applications found",
        );
    }
}
