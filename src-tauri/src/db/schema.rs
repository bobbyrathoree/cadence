use rusqlite::Connection;

/// Run all CREATE TABLE statements and set up indexes and FTS5.
pub fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        -- Prompts
        CREATE TABLE IF NOT EXISTS prompts (
            id              TEXT PRIMARY KEY,
            title           TEXT NOT NULL,
            description     TEXT,
            primary_variant_id TEXT,
            is_favorite     INTEGER DEFAULT 0,
            is_pinned       INTEGER DEFAULT 0,
            copy_count      INTEGER DEFAULT 0,
            last_copied_at  TEXT,
            created_at      TEXT,
            updated_at      TEXT,
            deleted_at      TEXT
        );

        -- Variants
        CREATE TABLE IF NOT EXISTS variants (
            id              TEXT PRIMARY KEY,
            prompt_id       TEXT NOT NULL,
            label           TEXT NOT NULL,
            content         TEXT NOT NULL,
            content_type    TEXT DEFAULT 'static',
            variables       TEXT,
            sort_order      INTEGER DEFAULT 0,
            created_at      TEXT,
            updated_at      TEXT,
            deleted_at      TEXT,
            FOREIGN KEY (prompt_id) REFERENCES prompts(id) ON DELETE CASCADE
        );

        -- Tags
        CREATE TABLE IF NOT EXISTS tags (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL UNIQUE,
            color           TEXT,
            created_at      TEXT
        );

        -- Prompt-Tags join table
        CREATE TABLE IF NOT EXISTS prompt_tags (
            prompt_id       TEXT NOT NULL,
            tag_id          TEXT NOT NULL,
            PRIMARY KEY (prompt_id, tag_id),
            FOREIGN KEY (prompt_id) REFERENCES prompts(id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id)    REFERENCES tags(id) ON DELETE CASCADE
        );

        -- Collections
        CREATE TABLE IF NOT EXISTS collections (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            description     TEXT,
            icon            TEXT,
            color           TEXT,
            is_smart        INTEGER DEFAULT 0,
            filter_query    TEXT,
            sort_field      TEXT,
            sort_order      TEXT DEFAULT 'asc',
            created_at      TEXT,
            updated_at      TEXT
        );

        -- Collection-Prompts join table
        CREATE TABLE IF NOT EXISTS collection_prompts (
            collection_id   TEXT NOT NULL,
            prompt_id       TEXT NOT NULL,
            position        INTEGER DEFAULT 0,
            PRIMARY KEY (collection_id, prompt_id),
            FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
            FOREIGN KEY (prompt_id)     REFERENCES prompts(id) ON DELETE CASCADE
        );

        -- Copy history
        CREATE TABLE IF NOT EXISTS copy_history (
            id              TEXT PRIMARY KEY,
            prompt_id       TEXT NOT NULL,
            variant_id      TEXT,
            copied_at       TEXT,
            metadata        TEXT,
            FOREIGN KEY (prompt_id) REFERENCES prompts(id) ON DELETE CASCADE
        );

        -- Playbooks
        CREATE TABLE IF NOT EXISTS playbooks (
            id              TEXT PRIMARY KEY,
            title           TEXT NOT NULL,
            description     TEXT,
            created_at      TEXT,
            updated_at      TEXT
        );

        -- Playbook steps
        CREATE TABLE IF NOT EXISTS playbook_steps (
            id              TEXT PRIMARY KEY,
            playbook_id     TEXT NOT NULL,
            prompt_id       TEXT,
            position        INTEGER NOT NULL,
            step_type       TEXT DEFAULT 'single',
            instructions    TEXT,
            choice_prompt_ids TEXT,
            UNIQUE(playbook_id, position),
            FOREIGN KEY (playbook_id) REFERENCES playbooks(id) ON DELETE CASCADE,
            FOREIGN KEY (prompt_id)   REFERENCES prompts(id) ON DELETE SET NULL
        );

        -- Playbook sessions (singleton row)
        CREATE TABLE IF NOT EXISTS playbook_sessions (
            id                  INTEGER PRIMARY KEY DEFAULT 1,
            active_playbook_id  TEXT,
            current_step        INTEGER DEFAULT 0,
            started_at          TEXT,
            FOREIGN KEY (active_playbook_id) REFERENCES playbooks(id) ON DELETE SET NULL
        );

        -- FTS mapping: stable rowid for each prompt_id
        CREATE TABLE IF NOT EXISTS fts_mapping (
            rowid       INTEGER PRIMARY KEY AUTOINCREMENT,
            prompt_id   TEXT NOT NULL UNIQUE
        );

        -- FTS5 virtual table for full-text search (contentless)
        CREATE VIRTUAL TABLE IF NOT EXISTS prompts_fts USING fts5(
            title,
            description,
            content,
            tags,
            content='',
            contentless_delete=1
        );

        -- Ensure prompt primary variants always reference an active variant owned by the prompt.
        CREATE TRIGGER IF NOT EXISTS validate_prompt_primary_variant
        BEFORE UPDATE OF primary_variant_id ON prompts
        FOR EACH ROW
        WHEN NEW.primary_variant_id IS NOT NULL
         AND NOT EXISTS (
             SELECT 1
             FROM variants
             WHERE id = NEW.primary_variant_id
               AND prompt_id = NEW.id
               AND deleted_at IS NULL
         )
        BEGIN
            SELECT RAISE(ABORT, 'primary_variant_id must reference an active prompt-owned variant');
        END;

        -- Prevent soft-deleting whichever variant a prompt currently treats as primary.
        CREATE TRIGGER IF NOT EXISTS prevent_primary_variant_soft_delete
        BEFORE UPDATE OF deleted_at ON variants
        FOR EACH ROW
        WHEN NEW.deleted_at IS NOT NULL
         AND OLD.deleted_at IS NULL
         AND EXISTS (
             SELECT 1
             FROM prompts
             WHERE id = OLD.prompt_id
               AND primary_variant_id = OLD.id
               AND deleted_at IS NULL
         )
        BEGIN
            SELECT RAISE(ABORT, 'cannot soft delete a prompt primary variant');
        END;

        -- Settings (key-value store for app preferences)
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        -- Indexes
        CREATE INDEX IF NOT EXISTS idx_prompts_deleted_at     ON prompts(deleted_at);
        CREATE INDEX IF NOT EXISTS idx_prompts_is_favorite    ON prompts(is_favorite);
        CREATE INDEX IF NOT EXISTS idx_prompts_last_copied_at ON prompts(last_copied_at);
        CREATE INDEX IF NOT EXISTS idx_variants_prompt_id     ON variants(prompt_id);
        CREATE INDEX IF NOT EXISTS idx_copy_history_prompt_id ON copy_history(prompt_id);
        CREATE INDEX IF NOT EXISTS idx_playbook_steps_playbook_id ON playbook_steps(playbook_id);
        ",
    )?;

    Ok(())
}
