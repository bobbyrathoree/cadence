use rusqlite::{params, Connection, OptionalExtension};

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::tag::{CreateTagRequest, Tag};
use crate::services::{prompt_service, transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagCountRow {
    pub name: String,
    pub color: Option<String>,
    pub prompt_count: i64,
}

pub fn get_or_create_tag(conn: &mut Db, name: &str) -> AppResult<Tag> {
    transaction::immediate(conn, |tx| get_or_create_tag_tx(tx, name))
}

pub(crate) fn get_or_create_tag_tx(conn: &Connection, name: &str) -> AppResult<Tag> {
    let existing = conn
        .query_row(
            "SELECT id, name, color, created_at FROM tags WHERE name = ?1",
            params![name],
            |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    created_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(AppError::from)?;

    if let Some(tag) = existing {
        return Ok(tag);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO tags (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![id, name, now],
    )
    .map_err(AppError::from)?;
    Ok(Tag {
        id,
        name: name.to_string(),
        color: None,
        created_at: Some(now),
    })
}

pub fn create_or_update_tag(conn: &mut Db, request: CreateTagRequest) -> AppResult<Tag> {
    transaction::immediate(conn, |tx| {
        let request = request.clone();
        let mut tag = get_or_create_tag_tx(tx, &request.name)?;
        if let Some(color) = request.color {
            let affected = tx
                .execute(
                    "UPDATE tags SET color = ?1 WHERE id = ?2",
                    params![color, tag.id],
                )
                .map_err(AppError::from)?;
            transaction::expect_one(affected)?;
            tag.color = Some(color);
        }
        Ok(tag)
    })
}

pub fn list_tags(conn: &Connection) -> AppResult<Vec<Tag>> {
    let mut stmt = conn
        .prepare("SELECT id, name, color, created_at FROM tags ORDER BY name")
        .map_err(AppError::from)?;
    let rows = stmt.query_map([], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)
}

pub fn list_tags_with_counts(conn: &Connection) -> AppResult<Vec<TagCountRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT t.name, t.color, COUNT(p.id)
             FROM tags t
             LEFT JOIN prompt_tags pt ON pt.tag_id = t.id
             LEFT JOIN prompts p ON p.id = pt.prompt_id AND p.deleted_at IS NULL
             GROUP BY t.id, t.name, t.color
             ORDER BY t.name ASC",
        )
        .map_err(AppError::from)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TagCountRow {
                name: row.get(0)?,
                color: row.get(1)?,
                prompt_count: row.get(2)?,
            })
        })
        .map_err(AppError::from)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)
}

pub fn get_tags_for_prompt(conn: &Connection, prompt_id: &str) -> AppResult<Vec<Tag>> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, t.color, t.created_at
             FROM tags t
             JOIN prompt_tags pt ON pt.tag_id = t.id
             WHERE pt.prompt_id = ?1
             ORDER BY t.name",
        )
        .map_err(AppError::from)?;
    let rows = stmt.query_map(params![prompt_id], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)
}

pub fn add_tags_to_prompt(
    conn: &mut Db,
    prompt_id: &str,
    tag_names: &[String],
) -> AppResult<Vec<Tag>> {
    transaction::immediate(conn, |tx| {
        let tags = add_tags_to_prompt_tx(tx, prompt_id, tag_names)?;
        prompt_service::update_fts_index_tx(tx, prompt_id)?;
        Ok(tags)
    })
}

pub(crate) fn add_tags_to_prompt_tx(
    conn: &Connection,
    prompt_id: &str,
    tag_names: &[String],
) -> AppResult<Vec<Tag>> {
    conn.query_row(
        "SELECT 1 FROM prompts WHERE id = ?1 AND deleted_at IS NULL",
        params![prompt_id],
        |_| Ok(()),
    )
    .map_err(AppError::from)?;

    let mut tags = Vec::new();
    for name in tag_names {
        let tag = get_or_create_tag_tx(conn, name)?;
        conn.execute(
            "INSERT OR IGNORE INTO prompt_tags (prompt_id, tag_id) VALUES (?1, ?2)",
            params![prompt_id, tag.id],
        )
        .map_err(AppError::from)?;
        tags.push(tag);
    }
    Ok(tags)
}

pub fn remove_tag_from_prompt(conn: &mut Db, prompt_id: &str, tag_id: &str) -> AppResult<()> {
    transaction::immediate(conn, |tx| {
        tx.query_row(
            "SELECT 1 FROM prompts WHERE id = ?1 AND deleted_at IS NULL",
            params![prompt_id],
            |_| Ok(()),
        )
        .map_err(AppError::from)?;
        tx.query_row("SELECT 1 FROM tags WHERE id = ?1", params![tag_id], |_| {
            Ok(())
        })
        .map_err(AppError::from)?;

        let affected = tx
            .execute(
                "DELETE FROM prompt_tags WHERE prompt_id = ?1 AND tag_id = ?2",
                params![prompt_id, tag_id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
        prompt_service::update_fts_index_tx(tx, prompt_id)
    })
}
