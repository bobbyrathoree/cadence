use rusqlite::{params, Connection};

use crate::models::tag::Tag;

/// Find a tag by name, or create a new one if it doesn't exist.
pub fn get_or_create_tag(conn: &Connection, name: &str) -> rusqlite::Result<Tag> {
    let mut stmt = conn.prepare("SELECT id, name, color, created_at FROM tags WHERE name = ?1")?;
    let tag = stmt.query_row(params![name], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            created_at: row.get(3)?,
        })
    });

    match tag {
        Ok(t) => Ok(t),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO tags (id, name, created_at) VALUES (?1, ?2, ?3)",
                params![id, name, now],
            )?;
            Ok(Tag {
                id,
                name: name.to_string(),
                color: None,
                created_at: Some(now),
            })
        }
        Err(e) => Err(e),
    }
}

/// List all tags ordered by name.
pub fn list_tags(conn: &Connection) -> rusqlite::Result<Vec<Tag>> {
    let mut stmt = conn.prepare("SELECT id, name, color, created_at FROM tags ORDER BY name")?;
    let rows = stmt.query_map([], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;

    let mut tags = Vec::new();
    for row in rows {
        tags.push(row?);
    }
    Ok(tags)
}

/// Get all tags for a specific prompt via the prompt_tags join table.
pub fn get_tags_for_prompt(conn: &Connection, prompt_id: &str) -> rusqlite::Result<Vec<Tag>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color, t.created_at
         FROM tags t
         JOIN prompt_tags pt ON pt.tag_id = t.id
         WHERE pt.prompt_id = ?1
         ORDER BY t.name",
    )?;
    let rows = stmt.query_map(params![prompt_id], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;

    let mut tags = Vec::new();
    for row in rows {
        tags.push(row?);
    }
    Ok(tags)
}

/// Add tags to a prompt. Each tag is created if it doesn't exist.
/// Returns the list of tags that were added.
pub fn add_tags_to_prompt(
    conn: &Connection,
    prompt_id: &str,
    tag_names: &[String],
) -> rusqlite::Result<Vec<Tag>> {
    let mut tags = Vec::new();
    for name in tag_names {
        let tag = get_or_create_tag(conn, name)?;
        conn.execute(
            "INSERT OR IGNORE INTO prompt_tags (prompt_id, tag_id) VALUES (?1, ?2)",
            params![prompt_id, tag.id],
        )?;
        tags.push(tag);
    }
    Ok(tags)
}

/// Remove a tag from a prompt.
pub fn remove_tag_from_prompt(
    conn: &Connection,
    prompt_id: &str,
    tag_id: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM prompt_tags WHERE prompt_id = ?1 AND tag_id = ?2",
        params![prompt_id, tag_id],
    )?;
    Ok(())
}
