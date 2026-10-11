# Changelog

All notable changes to Filemelon are documented here.

## [0.1.3] - 2026-10-11

### Added

- Added a **Copy and synchronize** rule action for backup workflows.
- Added an opt-in **Include subdirectories** setting for every action. When enabled, relative subdirectory structure is preserved at the destination.
- Copy rules reuse the existing source, destination, filters, stability, scheduling, and manual-run settings.
- New files are copied, changed files are safely replaced after verification, and identical files are skipped.
- Source files are always preserved; destination-only files are left untouched.
- Added UI, persistence, import/export, run history, cancellation, and core tests for copy rules.

### Changed

- Updated application and core package versions to `0.1.3`.
- Added a Windows startup smoke test to the release build so broken packaged applications fail in CI before publication.
- Startup failures are written to `startup.log` and shown in a Windows error dialog instead of failing silently.
- GitHub Actions can sign releases when Windows certificate secrets are configured; unsigned builds remain supported when those secrets are absent.
