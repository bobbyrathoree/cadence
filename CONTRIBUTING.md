# Contributing to Cadence

Thanks for your interest in contributing to Cadence. This document covers the basics of getting set up and submitting changes.

## Development Setup

### Prerequisites

- Apple Silicon Mac running macOS 12.0 or later
- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) `^20.19.0 || >=22.12.0`
- npm

### Getting Started

```bash
git clone https://github.com/bobbyrathoree/cadence.git
cd cadence
npm ci
npm run tauri dev
```

This starts the Vite dev server and the Tauri app with hot reload. Rust changes trigger a recompile; React changes hot-reload instantly.

Use `npm run tauri dev` for functional testing. `npm run dev` serves the React frontend in a browser, where Tauri IPC, event, clipboard, and native-window APIs are unavailable; data-backed workflows will fail or remain empty there.

### Project Structure

```
src/                    # React frontend (TypeScript)
  components/           # UI components organized by feature
  lib/                  # Hooks, context, API wrapper, types
src-tauri/              # Rust backend
  src/
    api/                # axum HTTP server + routes
    commands/           # Tauri IPC command handlers
crates/core/             # Shared database, models, and services
crates/mcp/              # stdio MCP server and smoke client
```

### Build

```bash
npm run tauri build
```

Produces `Cadence.app` in `target/release/bundle/macos/`.

### Verify

Run the same checks used by CI:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npx tsc --noEmit
npm run build
npx vitest run
npx playwright test
scripts/check-versions.sh
```

## How to Contribute

### Reporting Bugs

Open an issue with:
- What you expected to happen
- What actually happened
- Steps to reproduce
- macOS version

### Suggesting Features

Open an issue describing the feature and why it would be useful. For larger features, let's discuss the approach before you start coding.

### Submitting Changes

1. Fork the repo and create a branch from `master`
2. Make your changes
3. Ensure both Rust and TypeScript compile cleanly:
   ```bash
   cargo test --manifest-path src-tauri/Cargo.toml
   npx tsc --noEmit
   npm run build
   ```
4. Open a pull request with a clear description of what you changed and why

### Code Style

- **Rust:** Follow standard Rust conventions. `cargo fmt` and `cargo clippy` should pass clean.
- **TypeScript/React:** Use the existing patterns. Tailwind for layout, CSS variables for theme colors. Keep components focused.
- **Commits:** Use conventional commit messages (`feat:`, `fix:`, `chore:`).

### Architecture Principles

- The Rust service layer is the source of truth. UI and API are views.
- All database access goes through services, never direct SQL in commands or routes.
- Tauri IPC commands and axum routes are thin wrappers around services.
- React hooks handle data fetching. Components handle rendering. Context handles navigation state.
- Keep files focused. If a file is growing large, it's doing too much.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
