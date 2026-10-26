# Changelog

## Unreleased

### Added

- `make build` builds and installs `cvmd` with `cargo install --path . --locked`;
  `make` and `make help` list available targets.
- `edit [ALIAS]` and `build [ALIAS]` find applications by company or position alias,
  with numbered selection when several match. `info` displays the aliases.
- `cvmd info` reports the active configuration, workspace paths, editor source,
  renderer availability, and current application's original company and position names.

### Changed

- Bare `edit` and `build` commands show their help and exit successfully without
  loading configuration. Pass a company or position alias to edit or build a CV.
- New application folders use `YYYY-MM-DD_HH-MM-SS.company_alias.position_alias`,
  with local time and second precision. Existing date-only and fractional-second
  folders remain supported.
