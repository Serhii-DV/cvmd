use super::*;

#[cfg(unix)]
#[test]
fn configured_pdf_filename_is_used_and_unsetting_restores_the_markdown_name() {
    let workspace = Workspace::new();
    let application = workspace.application("ACME", "Developer");
    fs::create_dir(application.join("output")).unwrap();
    let renderer = unix::executable(
        &workspace,
        "filename-renderer",
        "cat >/dev/null\nprintf '%s' '%PDF-1.7 named'",
    );
    workspace.configure("pdf", "renderer", renderer.to_str().unwrap());
    let markdown = fs::read(application.join("cv.md")).unwrap();
    let identity = fs::read(application.join("application.toml")).unwrap();
    let default_pdf = application.join("output/cv.pdf");
    fs::write(&default_pdf, "%PDF-1.7 existing default").unwrap();
    let previous_output = application.join("cv.pdf");
    fs::write(&previous_output, "%PDF-1.7 previous location").unwrap();

    for filename in ["Jane Doe CV.pdf", "Résumé 北欧.PDF"] {
        success(
            workspace
                .command()
                .args(["config", "pdf.filename", filename])
                .output()
                .unwrap(),
        );
        assert_eq!(
            success(
                workspace
                    .command()
                    .args(["config", "--get", "pdf.filename"])
                    .output()
                    .unwrap()
            ),
            filename
        );
        assert!(
            success(
                workspace
                    .command()
                    .args(["config", "--list"])
                    .output()
                    .unwrap()
            )
            .contains(&format!("pdf.filename={filename}"))
        );
        let destination = application.join("output").join(filename);
        let output = workspace
            .command()
            .args(["build", "--verbose", "acme"])
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains(&format!("PDF: {}", destination.display()))
        );
        assert_eq!(success(output), format!("Built {}", destination.display()));
        assert_eq!(fs::read(&destination).unwrap(), b"%PDF-1.7 named");
        assert_eq!(
            fs::read_to_string(&default_pdf).unwrap(),
            "%PDF-1.7 existing default"
        );
    }

    success(
        workspace
            .command()
            .args(["config", "--unset", "pdf.filename"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        success(
            workspace
                .command()
                .args(["build", "acme"])
                .output()
                .unwrap()
        ),
        format!("Built {}", default_pdf.display())
    );
    assert_eq!(fs::read(&default_pdf).unwrap(), b"%PDF-1.7 named");
    assert_eq!(
        fs::read(application.join("output/Jane Doe CV.pdf")).unwrap(),
        b"%PDF-1.7 named"
    );
    assert_eq!(fs::read(application.join("cv.md")).unwrap(), markdown);
    assert_eq!(
        fs::read_to_string(previous_output).unwrap(),
        "%PDF-1.7 previous location"
    );
    assert_eq!(
        fs::read(application.join("application.toml")).unwrap(),
        identity
    );
}

#[cfg(unix)]
#[test]
fn failed_renders_preserve_the_configured_pdf_and_leave_no_new_output() {
    let workspace = Workspace::new();
    let application = workspace.application("ACME", "Developer");
    fs::create_dir(application.join("output")).unwrap();
    workspace.configure("pdf", "filename", "My CV.pdf");
    let custom_pdf = application.join("output/My CV.pdf");
    let default_pdf = application.join("output/cv.pdf");
    fs::write(&custom_pdf, "%PDF-1.7 previous custom").unwrap();
    fs::write(&default_pdf, "%PDF-1.7 previous default").unwrap();
    let original_cv = fs::read(application.join("cv.md")).unwrap();
    let initial_count = fs::read_dir(&application).unwrap().count();
    let initial_output_count = fs::read_dir(application.join("output")).unwrap().count();
    for (name, script, message) in [
        (
            "failed-named-renderer",
            "printf 'partial PDF'\nexit 7",
            "PDF renderer exited unsuccessfully",
        ),
        (
            "invalid-named-renderer",
            "printf 'not a PDF'",
            "Renderer output is not a PDF",
        ),
    ] {
        let renderer = unix::executable(&workspace, name, script);
        workspace.configure("pdf", "renderer", renderer.to_str().unwrap());
        failure(
            workspace
                .command()
                .args(["build", "acme"])
                .output()
                .unwrap(),
            message,
        );
        assert_eq!(
            fs::read_to_string(&custom_pdf).unwrap(),
            "%PDF-1.7 previous custom"
        );
        assert_eq!(
            fs::read_to_string(&default_pdf).unwrap(),
            "%PDF-1.7 previous default"
        );
        assert_eq!(fs::read(application.join("cv.md")).unwrap(), original_cv);
        assert_eq!(fs::read_dir(&application).unwrap().count(), initial_count);
        assert_eq!(
            fs::read_dir(application.join("output")).unwrap().count(),
            initial_output_count
        );
    }
    workspace.configure("pdf", "filename", "New failed CV.pdf");
    failure(
        workspace
            .command()
            .args(["build", "acme"])
            .output()
            .unwrap(),
        "Renderer output is not a PDF",
    );
    assert!(!application.join("output/New failed CV.pdf").exists());
    assert_eq!(fs::read_dir(&application).unwrap().count(), initial_count);
    assert_eq!(
        fs::read_dir(application.join("output")).unwrap().count(),
        initial_output_count
    );
}

#[test]
fn blocked_output_directory_preserves_existing_files() {
    let workspace = Workspace::new();
    let application = workspace.application("ACME", "Developer");
    let markdown = fs::read(application.join("cv.md")).unwrap();
    let blocked_directory = application.join("output");
    fs::write(&blocked_directory, "existing user file").unwrap();
    let previous_pdf = application.join("cv.pdf");
    fs::write(&previous_pdf, "%PDF-1.7 existing").unwrap();
    workspace.configure("pdf", "renderer", "/nonexistent/renderer");
    failure(
        workspace
            .command()
            .args(["build", "acme"])
            .output()
            .unwrap(),
        "Cannot create PDF output directory",
    );
    assert_eq!(
        fs::read_to_string(blocked_directory).unwrap(),
        "existing user file"
    );
    assert_eq!(
        fs::read_to_string(previous_pdf).unwrap(),
        "%PDF-1.7 existing"
    );
    assert_eq!(fs::read(application.join("cv.md")).unwrap(), markdown);
}

#[test]
fn invalid_pdf_filenames_are_rejected_without_changing_config_or_existing_files() {
    let workspace = Workspace::new();
    let application = workspace.application("ACME", "Developer");
    fs::create_dir(application.join("output")).unwrap();
    let original_config = fs::read(&workspace.config).unwrap();
    let original_cv = fs::read(application.join("cv.md")).unwrap();
    let pdf = application.join("output/cv.pdf");
    fs::write(&pdf, "%PDF-1.7 previous").unwrap();
    for filename in [
        "",
        " ",
        ".",
        "..",
        "../outside.pdf",
        "nested/CV.pdf",
        "nested\\CV.pdf",
        "/absolute.pdf",
        "C:\\absolute.pdf",
        "cv.md",
        "application.toml",
        "resume",
        ".pdf",
    ] {
        let output = workspace
            .command()
            .args(["config", "pdf.filename", filename])
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "accepted invalid filename {filename:?}"
        );
        assert_eq!(fs::read(&workspace.config).unwrap(), original_config);
        // The same validation applies to settings edited directly in TOML.
        workspace.configure("pdf", "filename", filename);
        failure(
            workspace
                .command()
                .args(["build", "acme"])
                .output()
                .unwrap(),
            "pdf.filename",
        );
        assert_eq!(fs::read(application.join("cv.md")).unwrap(), original_cv);
        assert_eq!(fs::read_to_string(&pdf).unwrap(), "%PDF-1.7 previous");
        fs::write(&workspace.config, &original_config).unwrap();
    }
}
