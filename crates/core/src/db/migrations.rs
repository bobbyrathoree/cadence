use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

const CURRENT_SCHEMA_VERSION: i64 = 3;
const SHORTCUTS_KEY: &str = "keyboard_shortcuts";
const SEEDED_AT_KEY: &str = "seeded_at";

pub fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let current_version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

    for version in (current_version + 1)..=CURRENT_SCHEMA_VERSION {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

        match version {
            1 => rebuild_fts(&tx)?,
            2 => canonicalize_shortcuts(&tx)?,
            3 => backfill_seed_marker(&tx)?,
            _ => unreachable!("unknown schema migration"),
        }

        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }

    Ok(())
}

fn rebuild_fts(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM prompts_fts", [])?;
    tx.execute("DELETE FROM fts_mapping", [])?;

    let prompt_ids = {
        let mut stmt = tx.prepare("SELECT id FROM prompts WHERE deleted_at IS NULL ORDER BY id")?;
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids
    };

    for prompt_id in prompt_ids {
        index_prompt(tx, &prompt_id)?;
    }

    Ok(())
}

fn index_prompt(tx: &Transaction<'_>, prompt_id: &str) -> rusqlite::Result<()> {
    let (title, description, content): (String, Option<String>, String) = tx.query_row(
        "SELECT p.title, p.description, COALESCE(v.content, '')
         FROM prompts p
         LEFT JOIN variants v
           ON v.id = p.primary_variant_id
          AND v.prompt_id = p.id
          AND v.deleted_at IS NULL
         WHERE p.id = ?1 AND p.deleted_at IS NULL",
        params![prompt_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    let tags = {
        let mut stmt = tx.prepare(
            "SELECT t.name
             FROM tags t
             JOIN prompt_tags pt ON pt.tag_id = t.id
             WHERE pt.prompt_id = ?1
             ORDER BY t.name",
        )?;
        let names = stmt
            .query_map(params![prompt_id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .join(" ");
        names
    };

    tx.execute(
        "INSERT INTO fts_mapping (prompt_id) VALUES (?1)",
        params![prompt_id],
    )?;
    let rowid = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO prompts_fts(rowid, title, description, content, tags)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            rowid,
            title,
            description.as_deref().unwrap_or(""),
            content,
            tags
        ],
    )?;

    Ok(())
}

fn canonicalize_shortcuts(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    let stored: Option<String> = tx
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![SHORTCUTS_KEY],
            |row| row.get(0),
        )
        .optional()?;

    let Some(stored) = stored else {
        return Ok(());
    };
    let Ok(mut shortcuts) = serde_json::from_str::<HashMap<String, String>>(&stored) else {
        return Ok(());
    };

    let mut changed = false;
    for binding in shortcuts.values_mut() {
        let canonical = canonicalize_binding(binding);
        if canonical != *binding {
            *binding = canonical;
            changed = true;
        }
    }

    if changed {
        let value = serde_json::to_string(&shortcuts)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        tx.execute(
            "UPDATE settings SET value = ?1 WHERE key = ?2",
            params![value, SHORTCUTS_KEY],
        )?;
    }

    Ok(())
}

fn canonicalize_binding(binding: &str) -> String {
    binding
        .split('+')
        .map(|part| match part {
            "ArrowUp" => "Up",
            "ArrowDown" => "Down",
            "ArrowLeft" => "Left",
            "ArrowRight" => "Right",
            other => other,
        })
        .collect::<Vec<_>>()
        .join("+")
}

fn backfill_seed_marker(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    let prompt_count: i64 = tx.query_row("SELECT COUNT(*) FROM prompts", [], |row| row.get(0))?;
    if prompt_count == 0 {
        return Ok(());
    }

    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
        params![SEEDED_AT_KEY, now],
    )?;
    Ok(())
}
