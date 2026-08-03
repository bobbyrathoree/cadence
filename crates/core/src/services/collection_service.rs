use rusqlite::{params, Connection, OptionalExtension};

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::collection::{Collection, CreateCollectionRequest};
use crate::models::prompt::PromptListItem;
use crate::services::{pagination, tag_service, transaction};

pub fn create_collection(conn: &mut Db, request: CreateCollectionRequest) -> AppResult<Collection> {
    transaction::immediate(conn, |tx| create_collection_tx(tx, request.clone()))
}

pub(crate) fn create_collection_tx(
    conn: &Connection,
    request: CreateCollectionRequest,
) -> AppResult<Collection> {
    let CreateCollectionRequest {
        name,
        description,
        icon,
        color,
        is_smart,
        filter_query,
    } = request;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO collections
            (id, name, description, icon, color, is_smart, filter_query, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            id,
            name,
            description,
            icon,
            color,
            is_smart as i64,
            filter_query,
            now,
        ],
    )
    .map_err(AppError::from)?;

    Ok(Collection {
        id,
        name,
        description,
        icon,
        color,
        is_smart,
        filter_query,
        sort_field: None,
        sort_order: Some("asc".to_string()),
        created_at: Some(now.clone()),
        updated_at: Some(now),
    })
}

/// List all collections.
pub fn list_collections(conn: &Connection) -> AppResult<Vec<Collection>> {
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
) -> AppResult<Vec<PromptListItem>> {
    let (limit, offset) = pagination::sanitize_pagination(Some(limit), Some(offset))?;
    get_collection_prompts_sanitized(conn, collection_id, limit, offset)
}

pub fn get_collection_prompts_page(
    conn: &Connection,
    collection_id: &str,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AppResult<Vec<PromptListItem>> {
    let (limit, offset) = pagination::sanitize_pagination(limit, offset)?;
    get_collection_prompts_sanitized(conn, collection_id, limit, offset)
}

fn get_collection_prompts_sanitized(
    conn: &Connection,
    collection_id: &str,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<PromptListItem>> {
    // First determine if this is a smart collection
    let (is_smart, filter_query): (bool, Option<String>) = conn.query_row(
        "SELECT is_smart, filter_query FROM collections WHERE id = ?1",
        params![collection_id],
        |row| Ok((row.get::<_, i64>(0)? != 0, row.get::<_, Option<String>>(1)?)),
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
) -> AppResult<Vec<PromptListItem>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.title, p.description, p.is_favorite, p.copy_count, p.last_copied_at,
                COALESCE(SUBSTR(v.content, 1, 100), '') AS snippet,
                (SELECT COUNT(*) FROM variants WHERE prompt_id = p.id AND deleted_at IS NULL) AS variant_count
         FROM prompts p
         JOIN collection_prompts cp ON cp.prompt_id = p.id
         LEFT JOIN variants v ON v.id = p.primary_variant_id AND v.deleted_at IS NULL
         WHERE cp.collection_id = ?1 AND p.deleted_at IS NULL
         ORDER BY cp.position ASC, cp.prompt_id ASC
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
        let (
            id,
            title,
            description,
            is_favorite,
            copy_count,
            last_copied_at,
            snippet,
            variant_count,
        ) = row?;
        let tags = tag_service::get_tags_for_prompt(conn, &id)?;
        items.push(PromptListItem {
            id,
            title,
            description,
            snippet,
            snippet_runs: Vec::new(),
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
/// Builds parameterized queries to prevent SQL injection.
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
) -> AppResult<Vec<PromptListItem>> {
    let filter_query = match filter_query {
        Some(q) if !q.is_empty() => q,
        _ => {
            // No filter, return empty
            return Ok(Vec::new());
        }
    };

    // Parse the JSON filter
    let filter: serde_json::Value = serde_json::from_str(filter_query)
        .map_err(|error| AppError::invalid(format!("Invalid filter_query JSON: {error}")))?;

    let conditions = filter["conditions"]
        .as_array()
        .ok_or_else(|| AppError::invalid("filter_query missing 'conditions' array"))?;

    let match_mode = filter["match"].as_str().unwrap_or("all");
    let joiner = if match_mode == "any" { " OR " } else { " AND " };

    let mut where_clauses: Vec<String> = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    // The first two positional params (?1, ?2) are reserved for LIMIT and OFFSET
    let mut param_index: usize = 2;

    for condition in conditions {
        let field = condition["field"].as_str().unwrap_or("");
        let op = condition["op"].as_str().unwrap_or("");

        match (field, op) {
            ("tag", "includes") => {
                if let Some(tag_value) = condition["value"].as_str() {
                    param_index += 1;
                    where_clauses.push(format!(
                        "EXISTS (SELECT 1 FROM prompt_tags pt JOIN tags t ON pt.tag_id = t.id WHERE pt.prompt_id = p.id AND t.name = ?{})",
                        param_index
                    ));
                    param_values.push(Box::new(tag_value.to_string()));
                }
            }
            ("is_favorite", "eq") => {
                param_index += 1;
                let val: i64 = if condition["value"].as_bool().unwrap_or(false) {
                    1
                } else {
                    0
                };
                where_clauses.push(format!("p.is_favorite = ?{}", param_index));
                param_values.push(Box::new(val));
            }
            ("is_pinned", "eq") => {
                param_index += 1;
                let val: i64 = if condition["value"].as_bool().unwrap_or(false) {
                    1
                } else {
                    0
                };
                where_clauses.push(format!("p.is_pinned = ?{}", param_index));
                param_values.push(Box::new(val));
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
         ORDER BY p.updated_at DESC, p.id DESC
         LIMIT ?1 OFFSET ?2",
        where_clause
    );

    let mut stmt = conn.prepare(&sql)?;

    // Build the full params list: [limit, offset, ...condition_values]
    let mut all_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    all_params.push(Box::new(limit));
    all_params.push(Box::new(offset));
    all_params.extend(param_values);

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        all_params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
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
        let (
            id,
            title,
            description,
            is_favorite,
            copy_count,
            last_copied_at,
            snippet,
            variant_count,
        ) = row?;
        let tags = tag_service::get_tags_for_prompt(conn, &id)?;
        items.push(PromptListItem {
            id,
            title,
            description,
            snippet,
            snippet_runs: Vec::new(),
            is_favorite,
            variant_count,
            copy_count,
            last_copied_at,
            tags,
        });
    }

    Ok(items)
}

pub fn add_prompt_to_collection(
    conn: &mut Db,
    collection_id: &str,
    prompt_id: &str,
) -> AppResult<()> {
    transaction::immediate(conn, |tx| {
        add_prompt_to_collection_tx(tx, collection_id, prompt_id)
    })
}

pub(crate) fn add_prompt_to_collection_tx(
    conn: &Connection,
    collection_id: &str,
    prompt_id: &str,
) -> AppResult<()> {
    ensure_manual_collection(conn, collection_id)?;
    ensure_active_prompt(conn, prompt_id)?;

    let exists = conn
        .query_row(
            "SELECT 1 FROM collection_prompts
             WHERE collection_id = ?1 AND prompt_id = ?2",
            params![collection_id, prompt_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(AppError::from)?;
    if exists.is_some() {
        return Ok(());
    }

    let max_position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) FROM collection_prompts WHERE collection_id = ?1",
            params![collection_id],
            |row| row.get(0),
        )
        .map_err(AppError::from)?;

    conn.execute(
        "INSERT INTO collection_prompts (collection_id, prompt_id, position)
         VALUES (?1, ?2, ?3)",
        params![collection_id, prompt_id, max_position + 1],
    )
    .map_err(AppError::from)?;

    Ok(())
}

pub fn remove_prompt_from_collection(
    conn: &mut Db,
    collection_id: &str,
    prompt_id: &str,
) -> AppResult<()> {
    transaction::immediate(conn, |tx| {
        remove_prompt_from_collection_tx(tx, collection_id, prompt_id)
    })
}

pub(crate) fn remove_prompt_from_collection_tx(
    conn: &Connection,
    collection_id: &str,
    prompt_id: &str,
) -> AppResult<()> {
    ensure_manual_collection(conn, collection_id)?;
    ensure_active_prompt(conn, prompt_id)?;

    // Removing an absent membership is intentionally idempotent.
    conn.execute(
        "DELETE FROM collection_prompts WHERE collection_id = ?1 AND prompt_id = ?2",
        params![collection_id, prompt_id],
    )
    .map_err(AppError::from)?;
    Ok(())
}

fn ensure_manual_collection(conn: &Connection, collection_id: &str) -> AppResult<()> {
    let is_smart = conn
        .query_row(
            "SELECT is_smart FROM collections WHERE id = ?1",
            params![collection_id],
            |row| Ok(row.get::<_, i64>(0)? != 0),
        )
        .map_err(AppError::from)?;
    if is_smart {
        Err(AppError::invalid(
            "Smart collection membership cannot be edited",
        ))
    } else {
        Ok(())
    }
}

fn ensure_active_prompt(conn: &Connection, prompt_id: &str) -> AppResult<()> {
    conn.query_row(
        "SELECT 1 FROM prompts WHERE id = ?1 AND deleted_at IS NULL",
        params![prompt_id],
        |_| Ok(()),
    )
    .map_err(AppError::from)
}
