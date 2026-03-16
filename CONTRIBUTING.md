# Contributing to Cadence

Thanks for your interest in contributing to Cadence. This document covers the basics of getting set up and submitting changes.

## Development Setup

### Prerequisites

- macOS (Cadence is a macOS-only app)
- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) v18+
- npm

### Getting Started

```bash
git clone https://github.com/yourusername/cadence.git
cd cadence
npm install
npm run tauri dev
```

This starts the Vite dev server and the Tauri app with hot reload. Rust changes trigger a recompile; React changes hot-reload instantly.

### Project Structure

```
src/                    # React frontend (TypeScript)
  components/           # UI components organized by feature
  lib/                  # Hooks, context, API wrapper, types
src-tauri/              # Rust backend
  src/
    api/                # axum HTTP server + routes
    commands/           # Tauri IPC command handlers
    db/                 # SQLite schema + connection
    models/             # Data structs
    services/           # Business logic (CRUD, search, import/export)
```

### Build

```bash
npm run tauri build
```

Produces `Cadence.app` in `src-tauri/target/release/bundle/macos/`.

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

1. Fork the repo and create a branch from `main`
2. Make your changes
3. Ensure both Rust and TypeScript compile cleanly:
   ```bash
   cargo build --manifest-path src-tauri/Cargo.toml
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
