use std::collections::HashMap;

use cadence_core::db::{migrate, schema};
use rusqlite::{params, Connection};

fn setup_v0_fixture() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    schema::create_tables(&conn).unwrap();

    conn.execute(
        "INSERT INTO prompts
            (id, title, description, primary_variant_id, is_favorite, is_pinned,
             copy_count, created_at, updated_at)
         VALUES ('prompt-1', 'Current title', 'Current description', NULL, 0, 0, 0,
                 '2026-01-01', '2026-01-01')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO variants
            (id, prompt_id, label, content, content_type, sort_order, created_at, updated_at)
         VALUES ('variant-1', 'prompt-1', 'Default', 'current searchable content',
                 'static', 0, '2026-01-01', '2026-01-01')",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE prompts SET primary_variant_id = 'variant-1' WHERE id = 'prompt-1'",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tags (id, name, created_at)
         VALUES ('tag-1', 'current-tag', '2026-01-01')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO prompt_tags (prompt_id, tag_id) VALUES ('prompt-1', 'tag-1')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO fts_mapping (rowid, prompt_id) VALUES (41, 'prompt-1')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO prompts_fts(rowid, title, description, content, tags)
         VALUES (41, 'stale title', '', 'redacted stale term', '')",
        [],
    )
    .unwrap();

    let shortcuts = serde_json::json!({
        "navigate_up": "ArrowUp",
        "navigate_down": "CommandOrControl+ArrowDown",
        "focus_search": "CommandOrControl+F"
    });
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('keyboard_shortcuts', ?1)",
        params![shortcuts.to_string()],
    )
    .unwrap();

    conn
}

fn fts_count(conn: &Connection, query: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM prompts_fts WHERE prompts_fts MATCH ?1",
        params![query],
        |row| row.get(0),
    )
    .unwrap()
}

fn user_version(conn: &Connection) -> i64 {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

#[test]
fn migrates_v0_fixture_and_is_idempotent() {
    let mut conn = setup_v0_fixture();

    migrate(&mut conn).unwrap();

    assert_eq!(user_version(&conn), 3);
    assert_eq!(fts_count(&conn, "redacted"), 0);
    assert_eq!(fts_count(&conn, "current"), 1);
    assert_eq!(fts_count(&conn, "\"current-tag\""), 1);

    let shortcuts: HashMap<String, String> = serde_json::from_str(
        &conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'keyboard_shortcuts'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(shortcuts["navigate_up"], "Up");
    assert_eq!(shortcuts["navigate_down"], "CommandOrControl+Down");
    assert_eq!(shortcuts["focus_search"], "CommandOrControl+F");

    let seeded_at: String = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'seeded_at'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    migrate(&mut conn).unwrap();

    assert_eq!(user_version(&conn), 3);
    assert_eq!(fts_count(&conn, "current"), 1);
    let seeded_at_after: String = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'seeded_at'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(seeded_at_after, seeded_at);
}

#[test]
fn failed_fts_migration_rolls_back_content_and_version_then_retries() {
    let mut conn = setup_v0_fixture();
    conn.execute_batch(
        "CREATE TRIGGER fail_after_fts_wipe
         BEFORE DELETE ON fts_mapping
         BEGIN
           SELECT RAISE(ABORT, 'injected migration failure');
         END;",
    )
    .unwrap();

    assert!(migrate(&mut conn).is_err());
    assert_eq!(user_version(&conn), 0);
    assert_eq!(fts_count(&conn, "redacted"), 1);
    assert_eq!(fts_count(&conn, "current"), 0);

    conn.execute_batch("DROP TRIGGER fail_after_fts_wipe")
        .unwrap();
    migrate(&mut conn).unwrap();

    assert_eq!(user_version(&conn), 3);
    assert_eq!(fts_count(&conn, "redacted"), 0);
    assert_eq!(fts_count(&conn, "current"), 1);
}
