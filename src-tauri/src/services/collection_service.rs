use rusqlite::{params, Connection};

use crate::models::collection::{Collection, CreateCollectionRequest};
use crate::models::prompt::PromptListItem;
use crate::services::tag_service;

/// Create a new collection.
pub fn create_collection(
    conn: &Connection,
    req: CreateCollectionRequest,
) -> rusqlite::Result<Collection> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO collections (id, name, description, icon, color, is_smart, filter_query, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            req.name,
            req.description,
            req.icon,
            req.color,
            req.is_smart as i64,
            req.filter_query,
            now,
            now,
        ],
    )?;

    Ok(Collection {
        id,
        name: req.name,
        description: req.description,
        icon: req.icon,
        color: req.color,
        is_smart: req.is_smart,
        filter_query: req.filter_query,
        sort_field: None,
        sort_order: Some("asc".to_string()),
        created_at: Some(now.clone()),
        updated_at: Some(now),
    })
}

/// List all collections.
pub fn list_collections(conn: &Connection) -> rusqlite::Result<Vec<Collection>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, icon, color, is_smart, filter_query,
                sort_field, sort_order, created_at, updated_at
         FROM collections
         ORDER BY name",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(Collection {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            icon: row.get(3)?,
            color: row.get(4)?,
            is_smart: row.get::<_, i64>(5)? != 0,
            filter_query: row.get(6)?,
            sort_field: row.get(7)?,
            sort_order: row.get(8)?,
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
        })
    })?;

    let mut collections = Vec::new();
    for row in rows {
        collections.push(row?);
    }
    Ok(collections)
}

/// Get prompts in a collection. For manual collections, joins through collection_prompts.
/// For smart collections, parses filter_query JSON and builds a dynamic query.
pub fn get_collection_prompts(
    conn: &Connection,
    collection_id: &str,
    limit: i64,
    offset: i64,
) -> rusqlite::Result<Vec<PromptListItem>> {
    // First determine if this is a smart collection
    let (is_smart, filter_query): (bool, Option<String>) = conn.query_row(
        "SELECT is_smart, filter_query FROM collections WHERE id = ?1",
        params![collection_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)? != 0,
                row.get::<_, Option<String>>(1)?,
            ))
        },
    )?;

    if is_smart {
        get_smart_collection_prompts(conn, filter_query.as_deref(), limit, offset)
    } else {
        get_manual_collection_prompts(conn, collection_id, limit, offset)
    }
}

/// Get prompts for a manual collection via the collection_prompts join table.
fn get_manual_collection_prompts(
    conn: &Connection,
    collection_id: &str,
    limit: i64,
    offset: i64,
) -> rusqlite::Result<Vec<PromptListItem>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.title, p.description, p.is_favorite, p.copy_count, p.last_copied_at,
                COALESCE(SUBSTR(v.content, 1, 100), '') AS snippet,
                (SELECT COUNT(*) FROM variants WHERE prompt_id = p.id AND deleted_at IS NULL) AS variant_count
         FROM prompts p
         JOIN collection_prompts cp ON cp.prompt_id = p.id
         LEFT JOIN variants v ON v.id = p.primary_variant_id AND v.deleted_at IS NULL
         WHERE cp.collection_id = ?1 AND p.deleted_at IS NULL
         ORDER BY cp.position
         LIMIT ?2 OFFSET ?3",
    )?;

    let rows = stmt.query_map(params![collection_id, limit, offset], |row| {
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

/// Get prompts for a smart collection by parsing the filter_query JSON.
///
/// filter_query format:
/// ```json
/// {
///   "conditions": [
///     {"field": "tag", "op": "includes", "value": "model:claude"},
///     {"field": "is_favorite", "op": "eq", "value": true}
///   ],
///   "match": "all"
/// }
/// ```
fn get_smart_collection_prompts(
    conn: &Connection,
    filter_query: Option<&str>,
    limit: i64,
    offset: i64,
) -> rusqlite::Result<Vec<PromptListItem>> {
    let filter_query = match filter_query {
        Some(q) if !q.is_empty() => q,
        _ => {
            // No filter, return empty
            return Ok(Vec::new());
        }
    };

    // Parse the JSON filter
    let filter: serde_json::Value = serde_json::from_str(filter_query).map_err(|e| {
        rusqlite::Error::InvalidParameterName(format!("Invalid filter_query JSON: {}", e))
    })?;

    let conditions = filter["conditions"]
        .as_array()
        .ok_or_else(|| {
            rusqlite::Error::InvalidParameterName("filter_query missing 'conditions' array".into())
        })?;

    let match_mode = filter["match"].as_str().unwrap_or("all");
    let joiner = if match_mode == "any" { " OR " } else { " AND " };

    let mut where_clauses: Vec<String> = Vec::new();

    for condition in conditions {
        let field = condition["field"].as_str().unwrap_or("");
        let op = condition["op"].as_str().unwrap_or("");

        match (field, op) {
            ("tag", "includes") => {
                if let Some(tag_value) = condition["value"].as_str() {
                    // Escape single quotes in tag name to prevent SQL injection
                    let escaped = tag_value.replace('\'', "''");
                    where_clauses.push(format!(
                        "EXISTS (SELECT 1 FROM prompt_tags pt JOIN tags t ON pt.tag_id = t.id WHERE pt.prompt_id = p.id AND t.name = '{}')",
                        escaped
                    ));
                }
            }
            ("is_favorite", "eq") => {
                let val = if condition["value"].as_bool().unwrap_or(false) {
                    1
                } else {
                    0
                };
                where_clauses.push(format!("p.is_favorite = {}", val));
            }
            ("is_pinned", "eq") => {
                let val = if condition["value"].as_bool().unwrap_or(false) {
                    1
                } else {
                    0
                };
                where_clauses.push(format!("p.is_pinned = {}", val));
            }
            _ => {
                // Unknown condition, skip
            }
        }
    }

    if where_clauses.is_empty() {
        return Ok(Vec::new());
    }

    let where_clause = where_clauses.join(joiner);

    let sql = format!(
        "SELECT p.id, p.title, p.description, p.is_favorite, p.copy_count, p.last_copied_at,
                COALESCE(SUBSTR(v.content, 1, 100), '') AS snippet,
                (SELECT COUNT(*) FROM variants WHERE prompt_id = p.id AND deleted_at IS NULL) AS variant_count
         FROM prompts p
         LEFT JOIN variants v ON v.id = p.primary_variant_id AND v.deleted_at IS NULL
         WHERE p.deleted_at IS NULL AND ({})
         ORDER BY p.updated_at DESC
         LIMIT ?1 OFFSET ?2",
        where_clause
    );

    let mut stmt = conn.prepare(&sql)?;

    let rows = stmt.query_map(params![limit, offset], |row| {
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

/// Add a prompt to a manual collection at the next position.
pub fn add_prompt_to_collection(
    conn: &Connection,
    collection_id: &str,
    prompt_id: &str,
) -> rusqlite::Result<()> {
    let max_position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) FROM collection_prompts WHERE collection_id = ?1",
            params![collection_id],
            |row| row.get(0),
        )?;

    conn.execute(
        "INSERT OR IGNORE INTO collection_prompts (collection_id, prompt_id, position) VALUES (?1, ?2, ?3)",
        params![collection_id, prompt_id, max_position + 1],
    )?;

    Ok(())
}

/// Remove a prompt from a collection.
pub fn remove_prompt_from_collection(
    conn: &Connection,
    collection_id: &str,
    prompt_id: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM collection_prompts WHERE collection_id = ?1 AND prompt_id = ?2",
        params![collection_id, prompt_id],
    )?;
    Ok(())
}
