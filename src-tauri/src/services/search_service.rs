use rusqlite::{params, Connection};

use crate::models::prompt::PromptListItem;
use crate::services::tag_service;

/// Search prompts using FTS5 full-text search.
/// Appends `*` to each word in the query for prefix matching.
/// Returns PromptListItem with snippet from FTS5 snippet() function.
///
/// Uses `fts_mapping` to join FTS results back to prompts via a simple
/// SQL JOIN, avoiding the previous O(N) hash-all-IDs reverse lookup.
pub fn search_prompts(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> rusqlite::Result<Vec<PromptListItem>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    // Sanitize the query: remove FTS5 special characters, then append * for prefix matching
    let sanitized = query
        .replace('"', "")
        .replace('\'', "")
        .replace('(', "")
        .replace(')', "")
        .replace('*', "")
        .replace('+', "")
        .replace('-', " ")
        .replace(':', " ");
    let sanitized = sanitized.trim();

    if sanitized.is_empty() {
        return Ok(Vec::new());
    }

    // Build prefix query: each word gets a * appended
    let fts_query: String = sanitized
        .split_whitespace()
        .map(|word| format!("{}*", word))
        .collect::<Vec<_>>()
        .join(" ");

    // Single query: join FTS results through fts_mapping to prompts
    let mut stmt = conn.prepare(
        "SELECT p.id, p.title, p.description, p.is_favorite, p.copy_count,
                p.last_copied_at,
                snippet(prompts_fts, 2, '<mark>', '</mark>', '...', 32) AS snippet,
                (SELECT COUNT(*) FROM variants WHERE prompt_id = p.id AND deleted_at IS NULL) AS variant_count
         FROM prompts_fts f
         JOIN fts_mapping m ON m.rowid = f.rowid
         JOIN prompts p ON p.id = m.prompt_id
         WHERE prompts_fts MATCH ?1 AND p.deleted_at IS NULL
         ORDER BY f.rank
         LIMIT ?2",
    )?;

    let rows = stmt.query_map(params![fts_query, limit], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, i64>(3)? != 0,
            row.get::<_, i64>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, i64>(7)?,
        ))
    })?;

    let mut items = Vec::new();
    for row in rows {
        let (id, title, description, is_favorite, copy_count, last_copied_at, snippet, variant_count) = row?;
        let tags = tag_service::get_tags_for_prompt(conn, &id)?;
        items.push(PromptListItem {
            id,
            title,
            description,
            snippet,
            is_favorite,
            variant_count,
            copy_count,
            last_copied_at,
            tags,
        });
    }

    Ok(items)
}
