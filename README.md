<p align="center"><img src="src/assets/branding/filemelon-logo.png" width="112" alt="Filemelon"></p>

# Filemelon

A local Windows file organizer. Configure move, copy, and sorting rules; Filemelon runs them on schedule and stays in the system tray.

## Installation

Choose one of these files in GitHub Releases:

- **`Filemelon-<version>-windows-x64-setup.exe`** — installer with WebView2, suitable for offline installation. Installs for the current user.
- **`Filemelon-<version>-windows-x64-portable.exe`** — runs without installation. Requires Microsoft Edge WebView2 Runtime. Rules and history are stored in AppData, so settings do not travel with the portable EXE.

Users do not need Node.js or Rust. The portable and installed versions share settings; only one instance runs at a time. After moving the portable file, disable and re-enable Windows autostart from the new location.

## Usage

1. Create a rule and choose a source folder.
2. Add conditions for extension, part of the name, size, creation age, or modification age. A rule can require all conditions or any condition.
3. Choose an action and destination:
   - **MOVE** — move files into one folder.
   - **COPY** — copy and synchronize files with a destination folder.
   - **SORT** — place files into subfolders using a path template.
4. Set a schedule and the minimum time since the last file change.
5. Enable the rule or click **Run now**.

Enable **Include subfolders** separately for each rule. When enabled, relative subfolder structure is preserved at the destination for move, copy, and sort actions. When disabled, only files directly inside the source folder are scanned.

The next run is shown in local time. Rules execute sequentially. A file belongs to the first enabled matching rule in creation order; manual runs use the same priority. Nested folders are scanned only when **Include subfolders** is enabled.

A **SORT** template can look like this:

```text
D:\Archive\{extension}\{year}\{month}
```

Available tokens are `{filename}` (name without extension), `{extension}`, `{year}`, `{month}`, `{day}`, `{created.year}`, `{created.month}`, and `{created.day}`. Ordinary date tokens use the execution date; `created.*` tokens use the file creation date. The template determines the destination directory while preserving the original file name.

Sizes use B/KB/MB/GB/TB, where 1 KB = 1024 B. Ages use seconds, minutes, hours, or days. Regardless of the configured age, a file must be stable for at least two seconds.

## File processing

- Within one drive, MOVE uses a native rename without rereading file contents.
- Across drives, MOVE copies with SHA-256 verification and removes the source only after successful verification.
- COPY compares SHA-256 content, skips identical files, and safely replaces changed destination files. Sources are never removed.
- On conflicts, identical content is recognized by SHA-256. For MOVE, an identical source is removed as a duplicate; different content receives `_1`, `_2`, and later suffixes.
- **Stop** cancels the current processing. Completed operations remain completed; copying and hashing observe cancellation between chunks. A partial copy is removed when cancelled.
- Disabled rules can be deleted together with their run history.
- Closing the window leaves the app in the tray. Use the tray menu to exit completely. Configure Windows autostart in the sidebar.

## Import, history, and data

JSON import and export use standard Windows dialogs. The format is `{ "version": 1, "rules": [...] }`. Import adds disabled rules without replacing existing rules; new identifiers are assigned. Check paths before enabling imported rules. Files are limited to 5 MiB and 1,000 rules, and import runs as one transaction after every record has been validated.

Settings and history are stored in `%APPDATA%\com.krabby.filemelon\app.db`. Detailed operations are written to `logs\operations-YYYY-MM-DD.jsonl` in the same directory. The latest 200 runs appear in the UI; old database rows and log files are not removed automatically. SQLite migrations are embedded in the executable and applied at startup. After an unexpected exit, unfinished runs are marked interrupted.

## Architecture

```text
Angular UI
    │ Tauri commands
Tauri desktop adapter ── native dialogs / tray / autostart
    │
filemelon-core (Rust)
    ├── conditions / templates / scheduler / cancellation
    ├── file operations / conflict resolution / daily log
    └── repository / SQLite migrations / run history
```

Angular handles display and input. Rules, scheduling, file validation, transfers, and persistence live in the independent `filemelon-core` Rust crate. The desktop adapter provides Windows integrations and serializes rule changes while tasks run. SQLx uses SQLite with only the required derive and runtime features. The logger keeps one file open until the day changes; disk synchronization runs after a rule starts. The scheduler reparses cron only when a rule changes or its next occurrence arrives.

```text
.github/workflows/     checks, Windows builds, and release publishing
scripts/               local builds and version checks
src/                   Angular UI and source logo
src-tauri/src/         Tauri desktop adapter
src-tauri/filemelon-core/ Rust core and tests
src-tauri/migrations/  immutable SQLite migrations
src-tauri/icons/       Windows icons generated from the logo
tests/                 UI behavior checks
```

## Development

Requirements: Node.js 24 LTS, stable Rust with the `x86_64-pc-windows-msvc` target, Visual Studio Build Tools with **Desktop development with C++**, Windows SDK, and WebView2. The first build needs internet access for dependencies and packaging tools.

```powershell
npm ci
npm run tauri -- dev
```

Checks:

```powershell
npm run check
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml -p filemelon-core --locked
cargo clippy --manifest-path src-tauri/Cargo.toml -p filemelon-core --all-targets --locked -- -D warnings
```

## Production build

```powershell
npm run build:prod
```

One command builds the release app, an NSIS installer with offline WebView2, and a standalone portable EXE. Outputs and `SHA256SUMS.txt` appear in `release/<version>-windows-x64/`. Use `npm run build:installer` or `npm run build:portable` for one variant; the portable build skips WebView2 packaging. The build stops on errors. Files from a previous successful build may remain in the directory, so check the command result and file dates.

## GitHub Actions and releases

`.github/workflows/windows-release.yml` runs UI and Rust checks, then builds both EXEs on Windows x64. It runs on pushes, pull requests, and manual dispatch from the Actions tab. Ordinary builds are available in the **Filemelon-windows-x64** artifact.

To publish a release:

1. Update the version in `package.json`, `src-tauri/tauri.conf.json`, and both `Cargo.toml` files.
2. Update `package-lock.json` and `src-tauri/Cargo.lock`, run the checks, and commit the changes.
3. Create and push a tag matching the version:

```powershell
git tag v0.1.3
git push origin v0.1.3
```

After a successful build, the workflow creates a GitHub Release and attaches the installer, portable build, and checksums. Tags with a suffix, such as `v0.2.0-beta.1`, are published as prereleases. Version checks stop a release when the tag does not match the configuration. The workflow uses the built-in `GITHUB_TOKEN`: read access for builds and write access only for publishing. No separate secret is required; repository policy must allow Releases through Actions.

## Current limitations

Windows x64 is supported. The UI editor supports one condition group; nested groups are supported by the core and JSON format but cannot be edited in the form. There is no automatic application update, automatic cleanup of history or logs, or release signing. Before public distribution, test installation, tray/autostart, and file transfers on a normal Windows machine.

### Rule management

- **Move to Recycle Bin** sends matching files to the Windows Recycle Bin so they can be restored.
- **Enable all** and **Disable all** apply the state to every rule.
- Priority follows list order. Drag a rule by the `⋮⋮` handle in card or compact view.
- Compact view, sidebar visibility, and language choice are saved locally in the app.