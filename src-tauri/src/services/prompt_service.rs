use rusqlite::{params, Connection, OptionalExtension};

use crate::models::prompt::{
    CreatePromptRequest, Prompt, PromptListItem, PromptWithVariants, UpdatePromptRequest, Variant,
};
use crate::services::tag_service;

fn invalid_input(message: impl Into<String>) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message.into())
}

fn ensure_variant_belongs_to_prompt(
    conn: &Connection,
    prompt_id: &str,
    variant_id: &str,
) -> rusqlite::Result<()> {
    let is_valid = conn
        .query_row(
            "SELECT 1
             FROM variants
             WHERE id = ?1 AND prompt_id = ?2 AND deleted_at IS NULL",
            params![variant_id, prompt_id],
            |_| Ok(()),
        )
        .optional()?;

    if is_valid.is_some() {
        Ok(())
    } else {
        Err(invalid_input(
            "variant must belong to the target prompt and remain active",
        ))
    }
}

fn get_active_primary_variant_id(conn: &Connection, prompt_id: &str) -> rusqlite::Result<String> {
    conn.query_row(
        "SELECT v.id
         FROM prompts p
         JOIN variants v ON v.id = p.primary_variant_id
         WHERE p.id = ?1
           AND p.deleted_at IS NULL
           AND v.prompt_id = p.id
           AND v.deleted_at IS NULL",
        params![prompt_id],
        |row| row.get(0),
    )
    .map_err(|err| match err {
        rusqlite::Error::QueryReturnedNoRows => {
            invalid_input("prompt primary variant is missing or invalid")
        }
        other => other,
    })
}

fn next_active_variant_id(
    conn: &Connection,
    prompt_id: &str,
    excluding_variant_id: &str,
) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT id
         FROM variants
         WHERE prompt_id = ?1 AND id <> ?2 AND deleted_at IS NULL
         ORDER BY sort_order, created_at, id
         LIMIT 1",
        params![prompt_id, excluding_variant_id],
        |row| row.get(0),
    )
    .optional()
}

/// Create a new prompt with a default variant, tags, and FTS index entry.
pub fn create_prompt(
    conn: &Connection,
    req: CreatePromptRequest,
) -> rusqlite::Result<PromptWithVariants> {
    let prompt_id = uuid::Uuid::new_v4().to_string();
    let variant_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let variant_label = req.variant_label.unwrap_or_else(|| "Default".to_string());

    // Insert the prompt
    conn.execute(
        "INSERT INTO prompts (id, title, description, primary_variant_id, is_favorite, is_pinned, copy_count, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?6, ?7)",
        params![
            prompt_id,
            req.title,
            req.description,
            Option::<String>::None,
            req.is_favorite as i64,
            now,
            now,
        ],
    )?;

    // Insert the default variant
    conn.execute(
        "INSERT INTO variants (id, prompt_id, label, content, content_type, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'static', 0, ?5, ?6)",
        params![variant_id, prompt_id, variant_label, req.content, now, now],
    )?;

    conn.execute(
        "UPDATE prompts SET primary_variant_id = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![variant_id, prompt_id],
    )?;

    // Add tags
    let tags = tag_service::add_tags_to_prompt(conn, &prompt_id, &req.tags)?;

    // Update FTS index
    update_fts_index(conn, &prompt_id)?;

    let prompt = Prompt {
        id: prompt_id,
        title: req.title,
        description: req.description,
        primary_variant_id: Some(variant_id.clone()),
        is_favorite: req.is_favorite,
        is_pinned: false,
        copy_count: 0,
        last_copied_at: None,
        created_at: Some(now.clone()),
        updated_at: Some(now.clone()),
        deleted_at: None,
    };

    let variant = Variant {
        id: variant_id,
        prompt_id: prompt.id.clone(),
        label: variant_label,
        content: req.content,
        content_type: Some("static".to_string()),
        variables: None,
        sort_order: 0,
        created_at: Some(now.clone()),
        updated_at: Some(now),
        deleted_at: None,
    };

    Ok(PromptWithVariants {
        prompt,
        variants: vec![variant],
        tags,
    })
}

/// Get a prompt by ID with all variants and tags.
pub fn get_prompt_by_id(conn: &Connection, id: &str) -> rusqlite::Result<PromptWithVariants> {
    let prompt = conn.query_row(
        "SELECT id, title, description, primary_variant_id, is_favorite, is_pinned,
                copy_count, last_copied_at, created_at, updated_at, deleted_at
         FROM prompts
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
        |row| {
            Ok(Prompt {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                primary_variant_id: row.get(3)?,
                is_favorite: row.get::<_, i64>(4)? != 0,
                is_pinned: row.get::<_, i64>(5)? != 0,
                copy_count: row.get(6)?,
                last_copied_at: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
                deleted_at: row.get(10)?,
            })
        },
    )?;

    // Fetch variants
    let mut stmt = conn.prepare(
        "SELECT id, prompt_id, label, content, content_type, variables, sort_order,
                created_at, updated_at, deleted_at
         FROM variants
         WHERE prompt_id = ?1 AND deleted_at IS NULL
         ORDER BY sort_order",
    )?;
    let variants = stmt
        .query_map(params![id], |row| {
            Ok(Variant {
                id: row.get(0)?,
                prompt_id: row.get(1)?,
                label: row.get(2)?,
                content: row.get(3)?,
                content_type: row.get(4)?,
                variables: row.get(5)?,
                sort_order: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                deleted_at: row.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let tags = tag_service::get_tags_for_prompt(conn, id)?;

    Ok(PromptWithVariants {
        prompt,
        variants,
        tags,
    })
}

/// List prompts with pagination. Returns a lightweight list item for each prompt.
pub fn list_prompts(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> rusqlite::Result<Vec<PromptListItem>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.title, p.description, p.is_favorite, p.copy_count, p.last_copied_at,
                COALESCE(
                    SUBSTR(v.content, 1, 100),
                    ''
                ) AS snippet,
                (SELECT COUNT(*) FROM variants WHERE prompt_id = p.id AND deleted_at IS NULL) AS variant_count
         FROM prompts p
         LEFT JOIN variants v ON v.id = p.primary_variant_id AND v.deleted_at IS NULL
         WHERE p.deleted_at IS NULL
         ORDER BY p.updated_at DESC
         LIMIT ?1 OFFSET ?2",
    )?;

    let rows = stmt.query_map(params![limit, offset], |row| {
        Ok((
            row.get::<_, String>(0)?,  // id
            row.get::<_, String>(1)?,  // title
            row.get::<_, Option<String>>(2)?,  // description
            row.get::<_, i64>(3)? != 0,  // is_favorite
            row.get::<_, i64>(4)?,  // copy_count
            row.get::<_, Option<String>>(5)?,  // last_copied_at
            row.get::<_, String>(6)?,  // snippet
            row.get::<_, i64>(7)?,  // variant_count
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

/// Update a prompt. Only non-None fields are updated.
pub fn update_prompt(
    conn: &Connection,
    id: &str,
    req: UpdatePromptRequest,
) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(ref title) = req.title {
        conn.execute(
            "UPDATE prompts SET title = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![title, now, id],
        )?;
    }
    if let Some(ref description) = req.description {
        conn.execute(
            "UPDATE prompts SET description = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![description, now, id],
        )?;
    }
    if let Some(is_favorite) = req.is_favorite {
        conn.execute(
            "UPDATE prompts SET is_favorite = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![is_favorite as i64, now, id],
        )?;
    }
    if let Some(is_pinned) = req.is_pinned {
        conn.execute(
            "UPDATE prompts SET is_pinned = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![is_pinned as i64, now, id],
        )?;
    }
    if let Some(ref primary_variant_id) = req.primary_variant_id {
        ensure_variant_belongs_to_prompt(conn, id, primary_variant_id)?;
        conn.execute(
            "UPDATE prompts SET primary_variant_id = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![primary_variant_id, now, id],
        )?;
    }

    // If no fields were set, still update updated_at
    if req.title.is_none()
        && req.description.is_none()
        && req.is_favorite.is_none()
        && req.is_pinned.is_none()
        && req.primary_variant_id.is_none()
    {
        conn.execute(
            "UPDATE prompts SET updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![now, id],
        )?;
    }

    // Rebuild FTS index if title or description changed
    if req.title.is_some() || req.description.is_some() {
        update_fts_index(conn, id)?;
    }

    Ok(())
}

/// Soft delete a prompt by setting deleted_at.
pub fn delete_prompt(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE prompts SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;

    // For FTS cleanup: the search_service already filters by deleted_at IS NULL
    // when joining FTS results back to prompts, so stale FTS entries for deleted
    // prompts are harmlessly ignored. We don't need to remove the FTS entry here.

    Ok(())
}

/// Record a copy event. Increments copy_count on the prompt, and returns the copied content.
pub fn record_copy(
    conn: &Connection,
    prompt_id: &str,
    variant_id: Option<&str>,
) -> rusqlite::Result<String> {
    let now = chrono::Utc::now().to_rfc3339();
    let copy_id = uuid::Uuid::new_v4().to_string();

    // Determine which variant to copy
    let actual_variant_id: String = match variant_id {
        Some(vid) => {
            ensure_variant_belongs_to_prompt(conn, prompt_id, vid)?;
            vid.to_string()
        }
        None => get_active_primary_variant_id(conn, prompt_id)?,
    };

    // Get the content
    let content: String = conn.query_row(
        "SELECT content
         FROM variants
         WHERE id = ?1 AND prompt_id = ?2 AND deleted_at IS NULL",
        params![actual_variant_id, prompt_id],
        |row| row.get(0),
    )?;

    // Insert copy history record
    conn.execute(
        "INSERT INTO copy_history (id, prompt_id, variant_id, copied_at) VALUES (?1, ?2, ?3, ?4)",
        params![copy_id, prompt_id, actual_variant_id, now],
    )?;

    // Increment copy_count and update last_copied_at
    conn.execute(
        "UPDATE prompts SET copy_count = copy_count + 1, last_copied_at = ?1, updated_at = ?1 WHERE id = ?2",
        params![now, prompt_id],
    )?;

    Ok(content)
}

/// Add a new variant to a prompt.
pub fn add_variant(
    conn: &Connection,
    prompt_id: &str,
    label: &str,
    content: &str,
) -> rusqlite::Result<Variant> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // Determine next sort_order
    let max_sort: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) FROM variants WHERE prompt_id = ?1 AND deleted_at IS NULL",
            params![prompt_id],
            |row| row.get(0),
        )?;

    conn.execute(
        "INSERT INTO variants (id, prompt_id, label, content, content_type, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'static', ?5, ?6, ?7)",
        params![id, prompt_id, label, content, max_sort + 1, now, now],
    )?;

    // Update the prompt's updated_at
    conn.execute(
        "UPDATE prompts SET updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, prompt_id],
    )?;

    // Rebuild FTS since variant content may be relevant
    update_fts_index(conn, prompt_id)?;

    Ok(Variant {
        id,
        prompt_id: prompt_id.to_string(),
        label: label.to_string(),
        content: content.to_string(),
        content_type: Some("static".to_string()),
        variables: None,
        sort_order: max_sort + 1,
        created_at: Some(now.clone()),
        updated_at: Some(now),
        deleted_at: None,
    })
}

/// Update a variant's content and optionally its label.
pub fn update_variant(
    conn: &Connection,
    id: &str,
    content: &str,
    label: Option<&str>,
) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(lbl) = label {
        conn.execute(
            "UPDATE variants SET content = ?1, label = ?2, updated_at = ?3 WHERE id = ?4 AND deleted_at IS NULL",
            params![content, lbl, now, id],
        )?;
    } else {
        conn.execute(
            "UPDATE variants SET content = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![content, now, id],
        )?;
    }

    // Get prompt_id for FTS rebuild
    let prompt_id: String = conn.query_row(
        "SELECT prompt_id FROM variants WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )?;

    // Update prompt's updated_at
    conn.execute(
        "UPDATE prompts SET updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, prompt_id],
    )?;

    update_fts_index(conn, &prompt_id)?;

    Ok(())
}

/// Soft delete a variant.
pub fn delete_variant(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    // Get prompt ownership before deleting.
    let (prompt_id, is_primary): (String, bool) = conn.query_row(
        "SELECT v.prompt_id,
                CASE WHEN p.primary_variant_id = v.id THEN 1 ELSE 0 END
         FROM variants v
         JOIN prompts p ON p.id = v.prompt_id
         WHERE v.id = ?1
           AND v.deleted_at IS NULL
           AND p.deleted_at IS NULL",
        params![id],
        |row| Ok((row.get(0)?, row.get::<_, i64>(1)? != 0)),
    )?;

    if is_primary {
        let replacement = next_active_variant_id(conn, &prompt_id, id)?.ok_or_else(|| {
            invalid_input("cannot delete the only active variant on a prompt")
        })?;

        conn.execute(
            "UPDATE prompts SET primary_variant_id = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![replacement, now, prompt_id],
        )?;
    }

    conn.execute(
        "UPDATE variants SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;

    if !is_primary {
        conn.execute(
            "UPDATE prompts SET updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![now, prompt_id],
        )?;
    }

    update_fts_index(conn, &prompt_id)?;

    Ok(())
}

/// Rebuild the FTS5 index entry for a given prompt.
/// Uses the `fts_mapping` table to maintain a stable rowid for each prompt_id,
/// avoiding the need for hashing and O(N) reverse lookups during search.
pub fn update_fts_index(conn: &Connection, prompt_id: &str) -> rusqlite::Result<()> {
    // Gather the data for this prompt
    let prompt_data: rusqlite::Result<(String, Option<String>)> = conn.query_row(
        "SELECT title, description FROM prompts WHERE id = ?1 AND deleted_at IS NULL",
        params![prompt_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );

    let (title, description) = match prompt_data {
        Ok(data) => data,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(()), // Prompt deleted or doesn't exist
        Err(e) => return Err(e),
    };

    // Get primary variant content
    let primary_content: String = conn
        .query_row(
            "SELECT COALESCE(v.content, '')
             FROM prompts p
             LEFT JOIN variants v ON v.id = p.primary_variant_id AND v.deleted_at IS NULL
             WHERE p.id = ?1 AND p.deleted_at IS NULL",
            params![prompt_id],
            |row| row.get(0),
        )
        .unwrap_or_default();

    // Get tag names
    let tags = tag_service::get_tags_for_prompt(conn, prompt_id)?;
    let tag_names: String = tags.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(" ");

    // Get or create a stable rowid via fts_mapping
    conn.execute(
        "INSERT OR IGNORE INTO fts_mapping (prompt_id) VALUES (?1)",
        params![prompt_id],
    )?;
    let rowid: i64 = conn.query_row(
        "SELECT rowid FROM fts_mapping WHERE prompt_id = ?1",
        params![prompt_id],
        |row| row.get(0),
    )?;

    // Delete old FTS entry (ignore errors if it doesn't exist yet)
    let _ = conn.execute(
        "INSERT INTO prompts_fts(prompts_fts, rowid, title, description, content, tags) VALUES('delete', ?1, ?2, ?3, ?4, ?5)",
        params![rowid, title, description.as_deref().unwrap_or(""), primary_content, tag_names],
    );

    // Insert new FTS entry
    conn.execute(
        "INSERT INTO prompts_fts(rowid, title, description, content, tags) VALUES(?1, ?2, ?3, ?4, ?5)",
        params![rowid, title, description.as_deref().unwrap_or(""), primary_content, tag_names],
    )?;

    Ok(())
}
