use rusqlite::{params, Connection};

use crate::models::prompt::PromptListItem;
use crate::services::tag_service;

/// Search prompts using FTS5 full-text search.
/// Appends `*` to each word in the query for prefix matching.
/// Returns PromptListItem with snippet from FTS5 snippet() function.
///
/// Since the FTS5 table is contentless, we use a deterministic hash of prompt_id
/// as the rowid. To map FTS results back to prompts, we reverse-lookup by
/// hashing all non-deleted prompt IDs.
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

    // Step 1: Query FTS to get matching rowids and content snippets
    let mut fts_stmt = conn.prepare(
        "SELECT rowid,
                snippet(prompts_fts, 2, '<mark>', '</mark>', '...', 32)
         FROM prompts_fts
         WHERE prompts_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2",
    )?;

    let fts_rows: Vec<(i64, String)> = fts_stmt
        .query_map(params![fts_query, limit], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if fts_rows.is_empty() {
        return Ok(Vec::new());
    }

    // Step 2: Build a reverse lookup from rowid (hash) -> prompt_id
    let mut id_stmt = conn.prepare("SELECT id FROM prompts WHERE deleted_at IS NULL")?;
    let all_ids: Vec<String> = id_stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let rowid_to_prompt: std::collections::HashMap<i64, String> = all_ids
        .iter()
        .map(|pid| (prompt_id_to_rowid(pid), pid.clone()))
        .collect();

    // Step 3: For each FTS result, look up the prompt and build list items
    let mut items = Vec::new();
    for (rowid, snippet) in &fts_rows {
        if let Some(prompt_id) = rowid_to_prompt.get(rowid) {
            let result = conn.query_row(
                "SELECT p.id, p.title, p.description, p.is_favorite, p.copy_count,
                        p.last_copied_at,
                        (SELECT COUNT(*) FROM variants WHERE prompt_id = p.id AND deleted_at IS NULL)
                 FROM prompts p
                 WHERE p.id = ?1 AND p.deleted_at IS NULL",
                params![prompt_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)? != 0,
                        row.get::<_, i64>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            );

            if let Ok((id, title, description, is_favorite, copy_count, last_copied_at, variant_count)) = result {
                let tags = tag_service::get_tags_for_prompt(conn, &id)?;
                items.push(PromptListItem {
                    id,
                    title,
                    description,
                    snippet: snippet.clone(),
                    is_favorite,
                    variant_count,
                    copy_count,
                    last_copied_at,
                    tags,
                });
            }
        }
    }

    Ok(items)
}

/// Convert a prompt UUID string to a deterministic i64 rowid for FTS5.
/// Must match the same function in prompt_service.
fn prompt_id_to_rowid(prompt_id: &str) -> i64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    prompt_id.hash(&mut hasher);
    (hasher.finish() & 0x7FFFFFFFFFFFFFFF) as i64
}
