# Cadence Architecture

This document records the v1.2 process, database, and service boundaries shared by the Tauri UI, local API, and MCP server.

## Process And Connection Topology

Cadence is an N-process local system over one SQLite database in WAL mode.

The application process owns:

1. A primary `DbAccess` connection for Tauri commands and lifecycle settings.
2. A dedicated read-only peer connection for the `PRAGMA data_version` poller.
3. An additional `DbAccess` connection while the opt-in local API is enabled.

Every configured MCP client starts its own `cadence-mcp` process. Each process owns one peer connection and sets `PRAGMA query_only=ON` except inside an explicit, write-gated `WriteScope`.

All connections enable foreign keys, use a 5-second busy timeout, and use `synchronous=NORMAL`. The application establishes WAL mode; peers require an up-to-date database to already be in WAL mode.

## Database Location And Openers

The default database is `dirs::data_dir()/Cadence/cadence.db`. MCP may override it with an absolute UTF-8 `CADENCE_DB_PATH`; the application always uses its admitted default path.

`open_app` opens read-write with create permission and may create the parent directory. `open_peer` opens read-write without create permission and returns `MissingFile` without creating a file or directory.

Both openers apply timeout and foreign-key pragmas, then read `user_version` before any mutating pragma:

- Version newer than the binary returns `SchemaNewer` without changing file bytes.
- Negative version returns `Corrupt` without changing file bytes.
- Versions below the current schema return `NeedsMigration`.
- The current version returns `Ready`; a peer additionally requires WAL mode.

Only the application migrates. MCP exits and asks the user to launch Cadence when migration is required.

## Transactions And Health

Service mutations use literal `BEGIN IMMEDIATE`, `COMMIT`, and observed `ROLLBACK` statements. Writer contention retries complete transaction cycles after 50 ms and 150 ms. Retry exhaustion returns a conflict without changing the database.

A failed commit always attempts rollback and verifies autocommit state. A rollback failure, non-autocommit connection, poisoned mutex, or panic inside a database closure makes that connection unrecoverable. `Health::poison` writes one fatal diagnostic and terminates its owning production process:

- The application and local API connections exit with code 1.
- MCP exits with code 10.

This is a crash-only recovery policy. SQLite WAL recovery handles the interrupted process on the next open. If the app crashes while its local API is enabled, `api.json` remains stale until the next app launch performs unconditional startup cleanup.

`DbAccess` is the only shared wrapper. It owns one mutex and one panic boundary; transports cannot access the mutex, guard, or connection ownership directly.

## Service Boundary

`cadence-core` owns validation, business rules, transaction boundaries, database mutation, FTS maintenance, and domain errors. Tauri commands, axum routes, and MCP protocol methods are transport views over the same services.

Transport code deserializes DTOs, calls a service contract, and maps the result. It does not introduce a second source of business truth. React owns drafts and presentation state, while persisted state is always rehydrated from a Rust service.

## MCP Transport

`cadence-mcp` uses newline-framed JSON-RPC over stdio. Stdout is protocol-only after startup; help and version output are the only pre-transport exceptions. Diagnostics always go to stderr.

Startup exit codes are:

| Code | Meaning |
|------|---------|
| 2 | Database file missing |
| 3 | Database schema requires app migration |
| 4 | Database schema is newer than the MCP binary |
| 5 | Corrupt database |
| 6 | Location or I/O failure |
| 8 | PRAGMA failure |
| 10 | Connection became unrecoverable while serving |

The server supports protocol versions `2025-11-25` and `2026-07-28`. It advertises tools, prompts, resources, and completion without list-changed notifications. Prompt and resource catalogs are capped at 100 entries; direct UUID resolution continues to work outside those catalogs. Full-text MCP search is also capped at 100 results.

MCP writes are absent unless `CADENCE_MCP_ALLOW_WRITES=1`. Release feature audits ensure `test-support`, `test-faults`, and lifecycle test features do not resolve into either shipping root, and that no resolved `cadence-core` unit enables `test-support`.

## Change Polling

The application poller owns its peer connection in one async task. It reads `PRAGMA data_version` every 500 ms while either app window is visible and emits `db-changed` when the value differs.

When both windows are hidden, it performs no database read and preserves its last-seen value. A commit made while hidden therefore emits on the first visible tick. Visibility query failures count as hidden. Three consecutive database read failures stop the poller with one warning; they do not crash the app.

## Local API Discovery

The optional API binds to `127.0.0.1`, uses a per-launch bearer key, and writes `api.json` with mode `0600`. Enable, startup, disable, and shutdown operations are serialized by `ApiLifecycle`.

v1.2 deliberately preserves v1.1's single-app-instance assumption. Concurrent Cadence installations share the same discovery path and may replace or remove each other's `api.json`. Single-instance enforcement and cross-process discovery ownership are deferred.

## Variant And Variable Models

A prompt is a metadata container. Prompt content lives in active `variants`; `primary_variant_id` selects the default. Copy, preview, search, import/export, Playbooks, and MCP resolve content through variants rather than introducing prompt-level content.

Rust and TypeScript share one variable grammar and one fixture corpus. `{{name}}` variables are fillable and `[PLACEHOLDER]` tokens are highlighted but not interpolated. Copy operations write the clipboard first, settle independently of usage accounting, and gate late UI effects by operation generation and component liveness.

## Full-Text Search

`prompts_fts` is a contentless FTS5 table. `fts_mapping` assigns stable integer FTS rowids to prompt UUIDs. Service mutations reindex flattened prompt metadata, active variant content, and tags; soft deletion removes both the FTS row and mapping.

Search is limit-only and capped at 100 results. List and collection views use deterministic offset pagination, which may repeat or skip an item if another writer mutates ordering between page requests. The UI deduplicates loaded prompt IDs, and refresh re-reads current ordering.

## Progressive Disclosure

Cadence exposes complexity in a ladder:

1. Library: browse, search, favorite, fill variables, and copy prompts.
2. Organization: add tags, variants, and manual collections.
3. Workflow: compose and run Playbooks.
4. Integration: opt into the local API or configure MCP clients.

Lower levels remain useful without configuring higher levels.
