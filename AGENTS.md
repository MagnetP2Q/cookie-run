# Repository Guidelines

## Project Structure & Module Organization

- `src/app/` contains the React interface; `src/lib/` holds backend calls and `src/types/` shared TypeScript types. Global styles live in `src/`.
- `src-tauri/src/` contains the Rust backend, organized into modules such as `adb`, `bot`, `config`, `games`, and `ocr`. Tauri configuration and icons are in `src-tauri/`.
- `config/cookie-run.default.json` supplies default rules; `public/` contains static assets and `tests/ocr-fixtures/` contains OCR images. `packaging/` holds the MSIX script.
- Treat `dist/`, `node_modules/`, and `src-tauri/target/` as generated output.

## Build, Test, and Development Commands

- `npm install` installs frontend dependencies.
- `npm run tauri dev` starts the desktop app in development mode. Windows Tauri prerequisites, Rust, ADB, and an Android emulator are required.
- `npm run dev` runs the Vite frontend; `npm run build` type-checks TypeScript and builds it; `npm run preview` serves the built frontend.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` runs Rust library tests.

## Coding Style & Naming Conventions

Follow the surrounding file's indentation and formatting. Use PascalCase for React components and TypeScript types, camelCase for functions and variables, and Rust's snake_case for functions and modules. Keep Rust formatted with `cargo fmt --manifest-path src-tauri/Cargo.toml`. No dedicated frontend lint or formatting script is configured; run the TypeScript build before submitting changes.

## Testing Guidelines

Rust tests are colocated with their modules under `#[cfg(test)]`; name tests descriptively in snake_case. Add focused tests for changed backend behavior and use the OCR fixtures where appropriate. For Windows OCR, the README documents an ignored synthetic-image test. The ignored live emulator test can tap the game and consume resources; run it only deliberately. No coverage threshold or frontend test runner is currently configured.

## Commits & Pull Requests

Use concise, imperative commit subjects describing the change. Git history was unavailable in this checkout, so no existing convention could be verified. Pull requests should explain the behavior changed, list verification commands and results, link relevant issues, and include screenshots for interface changes.

## Configuration & Packaging

App settings and editable `cookie-run.json` are stored in the Windows app-config directory; configure the ADB executable in the UI and use a 1600×900 emulator. MSIX packaging requires an appropriate identity and signing certificate; consult the README and keep certificate credentials out of the repository.

## Project Handoff

Read `AGENT.md` for current project state and `WORK_HISTORY.md` for the dated work history before continuing. Recheck current files and actual runtime state rather than assuming the handoff is still current.
