# Changelog

## Unreleased

### Added

- `build --verbose` (`-v`) reports the selected config, application Markdown, and
  configured stylesheet paths on stderr to help diagnose PDF inputs.
- `config --local` reads or updates the nearest workspace `cvmd.toml` and fails
  if none exists, preventing fallback to the user configuration.
- Git-style `config KEY [VALUE]`, `config --get KEY`, `config --list`, and
  `config --unset KEY` commands to read and update workspace settings. Set `source`
  to any CV source for future applications without changing existing CVs.

- `make build` builds and installs `cvmd` with `cargo install --path . --locked`;
  `make` and `make help` list available targets.
- `edit [ALIAS]` and `build [ALIAS]` find applications by company or position alias,
  with numbered selection when several match. `info` displays the aliases.
- `cvmd info` reports the active configuration, workspace paths, editor source,
  renderer availability, and current application's original company and position names.

### Changed

- New workspaces put the CV and shared stylesheet together in `assets/cv.md` and
  `assets/cv.css`. CV imports use `init --source FILE`; the CV input setting is
  `source`, including `config source` and the `Source CV` label in `info`.

- `init` now creates `cvmd.toml` in the workspace root with a portable relative
  root. Commands discover the nearest config in the current directory or its
  parents; `--config` takes priority and the old user config remains a fallback.
  After initializing, enter the workspace or pass `--config` when working elsewhere.
  To migrate existing settings, copy the old config into the workspace as
  `cvmd.toml` and run `cvmd config root .` there.

- Bare `edit` and `build` commands show their help and exit successfully without
  loading configuration. Pass a company or position alias to edit or build a CV.
- New application folders use `YYYY-MM-DD_HH-MM-SS.company_alias.position_alias`,
  with local time and second precision. Existing date-only and fractional-second
  folders remain supported.
