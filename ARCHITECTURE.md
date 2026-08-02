# Cadence Architecture

This document records the v1.1 boundaries that must remain consistent across the Tauri UI, local API, and SQLite storage.

## Variant Model

A prompt is a metadata container. It owns the title, description, favorite and pinned state, usage metadata, tags, and a `primary_variant_id`. Prompt text does not live on the prompt row.

Actual prompt content lives in `variants`. Each variant belongs to one prompt and carries its own label, content, content type, variables, ordering, and lifecycle timestamps. Copy, preview, search, import/export, and Playbook code must resolve content through variants. A transport that needs content hydrates the prompt with its active variants rather than introducing a second prompt-content source of truth.

## SQLite Connection Topology

Cadence uses two file-backed SQLite connections:

1. The primary connection serves Tauri IPC and app lifecycle work.
2. A second connection exists only while the opt-in local API server is running.

Both connections use the same database in WAL mode. WAL permits readers to proceed while another connection writes, but SQLite still has one writer at a time. Every connection must enable foreign keys and use `synchronous = NORMAL` plus `busy_timeout = 5000`.

The busy timeout is a correctness dependency of this topology, not a tuning detail. Without it, short service-owned write transactions on one connection can make valid writes on the other fail immediately with `SQLITE_BUSY`. Mutations therefore use bounded, explicit transactions and keep network or UI work outside the transaction.

## Service Boundary

The Rust service layer is the source of truth. It owns validation, authorization-independent business rules, transaction boundaries, database mutation, FTS maintenance, and domain errors.

Tauri commands and axum routes are transport views over the same services. They deserialize transport DTOs, call one service contract, map the result to IPC or HTTP semantics, and emit transport-specific events. They do not contain direct SQL or duplicate business decisions. React is another view: it may manage drafts and presentation state, but persisted truth comes back through a service transport.

## Progressive Disclosure

Cadence exposes complexity in a ladder:

1. **Library:** browse, search, favorite, and copy individual prompts.
2. **Organization:** add tags, variants, and manual collections as the library grows.
3. **Workflow:** compose repeated sequences into Playbooks and run one active session.

This is a product and architecture constraint. Lower levels must remain useful without configuring higher levels, and storage or service contracts must not require a user to adopt Collections or Playbooks before basic prompt workflows work.

## Pagination Limitation

List and collection views use deterministic offset pagination. Offset pagination is not stable under mutation: an insert, delete, or reorder between page requests can shift later offsets, causing one fetch to skip or repeat an item.

Cadence accepts this limitation for the v1.1 single-user local model. The UI deduplicates loaded pages by prompt ID, and a refresh re-reads the current ordering. Cursor pagination is deferred unless synchronization or multi-writer use makes mutation-stable traversal necessary.

## Full-text Search

`prompts_fts` is a contentless FTS5 table. SQLite does not copy canonical prompt data into or out of it automatically, and a contentless table cannot use an external-content `rebuild` as its recovery mechanism.

`fts_mapping` assigns a stable integer FTS rowid to each prompt ID. Search joins FTS matches through that mapping instead of treating a text UUID as an FTS rowid. Service mutations reindex the flattened searchable document when prompt metadata, non-deleted variant content, primary-variant selection, or tags change. Soft deletion evicts both the FTS row and mapping.

The relational prompt, variant, and tag tables remain canonical. Migrations repair the derived index by wiping and repopulating FTS rows and mappings in one transaction, advancing `PRAGMA user_version` only after the rebuild succeeds.
