use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{AppError, AppResult};
use crate::models::patch::PatchField;
use crate::models::prompt::{
    CreatePromptRequest, Prompt, PromptListItem, PromptUsage, PromptWithVariants,
    UpdatePromptRequest, Variant,
};
use crate::services::{tag_service, transaction};

fn ensure_prompt_exists(conn: &Connection, prompt_id: &str) -> AppResult<()> {
    conn.query_row(
        "SELECT 1 FROM prompts WHERE id = ?1 AND deleted_at IS NULL",
        params![prompt_id],
        |_| Ok(()),
    )
    .map_err(AppError::from)
}

fn ensure_variant_belongs_to_prompt(
    conn: &Connection,
    prompt_id: &str,
    variant_id: &str,
) -> AppResult<()> {
    conn.query_row(
        "SELECT 1
         FROM variants v
         JOIN prompts p ON p.id = v.prompt_id AND p.deleted_at IS NULL
         WHERE v.id = ?1
           AND v.prompt_id = ?2
           AND v.deleted_at IS NULL",
        params![variant_id, prompt_id],
        |_| Ok(()),
    )
    .map_err(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::Invalid("Variant must belong to the target prompt".to_string())
        }
        other => AppError::from(other),
    })
}

fn get_active_primary_variant_id(conn: &Connection, prompt_id: &str) -> AppResult<String> {
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
    .map_err(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::Invalid("Prompt primary variant is missing or invalid".to_string())
        }
        other => AppError::from(other),
    })
}

fn next_active_variant_id(
    conn: &Connection,
    prompt_id: &str,
    excluding_variant_id: &str,
) -> AppResult<Option<String>> {
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
    .map_err(AppError::from)
}

pub fn create_prompt(
    conn: &mut Connection,
    request: CreatePromptRequest,
) -> AppResult<PromptWithVariants> {
    transaction::immediate(conn, move |tx| create_prompt_tx(tx, request))
}

pub(crate) fn create_prompt_tx(
    conn: &Connection,
    request: CreatePromptRequest,
) -> AppResult<PromptWithVariants> {
    let CreatePromptRequest {
        title,
        description,
        content,
        variant_label,
        tags: tag_names,
        is_favorite,
    } = request;

    let prompt_id = uuid::Uuid::new_v4().to_string();
    let variant_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let variant_label = variant_label.unwrap_or_else(|| "Default".to_string());

    conn.execute(
        "INSERT INTO prompts
            (id, title, description, primary_variant_id, is_favorite, is_pinned,
             copy_count, created_at, updated_at)
         VALUES (?1, ?2, ?3, NULL, ?4, 0, 0, ?5, ?5)",
        params![prompt_id, title, description, is_favorite as i64, now],
    )
    .map_err(AppError::from)?;

    conn.execute(
        "INSERT INTO variants
            (id, prompt_id, label, content, content_type, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'static', 0, ?5, ?5)",
        params![variant_id, prompt_id, variant_label, content, now],
    )
    .map_err(AppError::from)?;

    let affected = conn
        .execute(
            "UPDATE prompts SET primary_variant_id = ?1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![variant_id, prompt_id],
        )
        .map_err(AppError::from)?;
    transaction::expect_one(affected)?;

    let tags = tag_service::add_tags_to_prompt_tx(conn, &prompt_id, &tag_names)?;
    update_fts_index_tx(conn, &prompt_id)?;

    Ok(PromptWithVariants {
        prompt: Prompt {
            id: prompt_id.clone(),
            title,
            description,
            primary_variant_id: Some(variant_id.clone()),
            is_favorite,
            is_pinned: false,
            copy_count: 0,
            last_copied_at: None,
            created_at: Some(now.clone()),
            updated_at: Some(now.clone()),
            deleted_at: None,
        },
        variants: vec![Variant {
            id: variant_id,
            prompt_id,
            label: variant_label,
            content,
            content_type: Some("static".to_string()),
            variables: None,
            sort_order: 0,
            created_at: Some(now.clone()),
            updated_at: Some(now),
            deleted_at: None,
        }],
        tags,
    })
}

pub fn get_prompt_by_id(conn: &Connection, id: &str) -> AppResult<PromptWithVariants> {
    let prompt = conn
        .query_row(
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
        )
        .map_err(AppError::from)?;

    let variants = {
        let mut stmt = conn
            .prepare(
                "SELECT id, prompt_id, label, content, content_type, variables, sort_order,
                        created_at, updated_at, deleted_at
                 FROM variants
                 WHERE prompt_id = ?1 AND deleted_at IS NULL
                 ORDER BY sort_order",
            )
            .map_err(AppError::from)?;
        let rows = stmt.query_map(params![id], |row| {
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
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };

    let tags = tag_service::get_tags_for_prompt(conn, id)?;
    Ok(PromptWithVariants {
        prompt,
        variants,
        tags,
    })
}

pub fn list_prompts(conn: &Connection, limit: i64, offset: i64) -> AppResult<Vec<PromptListItem>> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.title, p.description, p.is_favorite, p.copy_count,
                    p.last_copied_at, COALESCE(SUBSTR(v.content, 1, 100), ''),
                    (SELECT COUNT(*) FROM variants
                     WHERE prompt_id = p.id AND deleted_at IS NULL)
             FROM prompts p
             LEFT JOIN variants v ON v.id = p.primary_variant_id AND v.deleted_at IS NULL
             WHERE p.deleted_at IS NULL
             ORDER BY p.updated_at DESC
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(AppError::from)?;

    let rows = stmt
        .query_map(params![limit, offset], |row| {
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
        })
        .map_err(AppError::from)?;

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
        ) = row.map_err(AppError::from)?;
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

pub fn update_prompt(
    conn: &mut Connection,
    id: &str,
    request: UpdatePromptRequest,
) -> AppResult<()> {
    transaction::immediate(conn, |tx| update_prompt_tx(tx, id, request))
}

pub(crate) fn update_prompt_tx(
    conn: &Connection,
    id: &str,
    request: UpdatePromptRequest,
) -> AppResult<()> {
    ensure_prompt_exists(conn, id)?;
    let now = chrono::Utc::now().to_rfc3339();
    let should_reindex = request.title.is_some()
        || !request.description.is_keep()
        || request.primary_variant_id.is_some();

    if let Some(ref title) = request.title {
        let affected = conn
            .execute(
                "UPDATE prompts SET title = ?1, updated_at = ?2
                 WHERE id = ?3 AND deleted_at IS NULL",
                params![title, now, id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
    }
    match &request.description {
        PatchField::Keep => {}
        PatchField::Clear => {
            let affected = conn
                .execute(
                    "UPDATE prompts SET description = NULL, updated_at = ?1
                     WHERE id = ?2 AND deleted_at IS NULL",
                    params![now, id],
                )
                .map_err(AppError::from)?;
            transaction::expect_one(affected)?;
        }
        PatchField::Set(description) => {
            let affected = conn
                .execute(
                    "UPDATE prompts SET description = ?1, updated_at = ?2
                     WHERE id = ?3 AND deleted_at IS NULL",
                    params![description, now, id],
                )
                .map_err(AppError::from)?;
            transaction::expect_one(affected)?;
        }
    }
    if let Some(is_favorite) = request.is_favorite {
        let affected = conn
            .execute(
                "UPDATE prompts SET is_favorite = ?1, updated_at = ?2
                 WHERE id = ?3 AND deleted_at IS NULL",
                params![is_favorite as i64, now, id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
    }
    if let Some(is_pinned) = request.is_pinned {
        let affected = conn
            .execute(
                "UPDATE prompts SET is_pinned = ?1, updated_at = ?2
                 WHERE id = ?3 AND deleted_at IS NULL",
                params![is_pinned as i64, now, id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
    }
    if let Some(ref primary_variant_id) = request.primary_variant_id {
        ensure_variant_belongs_to_prompt(conn, id, primary_variant_id)?;
        let affected = conn
            .execute(
                "UPDATE prompts SET primary_variant_id = ?1, updated_at = ?2
                 WHERE id = ?3 AND deleted_at IS NULL",
                params![primary_variant_id, now, id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
    }

    if request.title.is_none()
        && request.description.is_keep()
        && request.is_favorite.is_none()
        && request.is_pinned.is_none()
        && request.primary_variant_id.is_none()
    {
        let affected = conn
            .execute(
                "UPDATE prompts SET updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
                params![now, id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
    }

    if should_reindex {
        update_fts_index_tx(conn, id)?;
    }
    Ok(())
}

pub fn get_prompt_usage(conn: &Connection, prompt_id: &str) -> AppResult<PromptUsage> {
    ensure_prompt_exists(conn, prompt_id)?;
    let rows = {
        let mut stmt = conn
            .prepare(
                "SELECT pb.title, COUNT(ps.id)
                 FROM playbooks pb
                 JOIN playbook_steps ps ON ps.playbook_id = pb.id
                 WHERE ps.prompt_id = ?1
                    OR INSTR(
                        ',' || COALESCE(ps.choice_prompt_ids, '') || ',',
                        ',' || ?1 || ','
                    ) > 0
                 GROUP BY pb.id, pb.title
                 ORDER BY pb.title ASC, pb.id ASC",
            )
            .map_err(AppError::from)?;
        let mapped = stmt
            .query_map(params![prompt_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
            })
            .map_err(AppError::from)?;
        mapped
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(AppError::from)?
    };
    Ok(PromptUsage {
        playbook_count: rows.len() as u32,
        step_count: rows.iter().map(|(_, count)| *count).sum(),
        playbook_titles: rows.into_iter().map(|(title, _)| title).collect(),
    })
}

pub fn toggle_favorite(conn: &mut Connection, id: &str) -> AppResult<bool> {
    toggle_flag(conn, id, "is_favorite")
}

pub fn toggle_pinned(conn: &mut Connection, id: &str) -> AppResult<bool> {
    toggle_flag(conn, id, "is_pinned")
}

fn toggle_flag(conn: &mut Connection, id: &str, column: &str) -> AppResult<bool> {
    transaction::immediate(conn, |tx| {
        let select_sql =
            format!("SELECT {column} FROM prompts WHERE id = ?1 AND deleted_at IS NULL");
        let current = tx
            .query_row(&select_sql, params![id], |row| {
                Ok(row.get::<_, i64>(0)? != 0)
            })
            .map_err(AppError::from)?;
        let new_state = !current;
        let update_sql = format!(
            "UPDATE prompts SET {column} = ?1, updated_at = ?2
             WHERE id = ?3 AND deleted_at IS NULL"
        );
        let affected = tx
            .execute(
                &update_sql,
                params![new_state as i64, chrono::Utc::now().to_rfc3339(), id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
        Ok(new_state)
    })
}

pub fn delete_prompt(conn: &mut Connection, id: &str) -> AppResult<()> {
    transaction::immediate(conn, |tx| {
        let now = chrono::Utc::now().to_rfc3339();
        let affected = tx
            .execute(
                "UPDATE prompts SET deleted_at = ?1, updated_at = ?1
                 WHERE id = ?2 AND deleted_at IS NULL",
                params![now, id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
        update_fts_index_tx(tx, id)
    })
}

pub fn record_copy(
    conn: &mut Connection,
    prompt_id: &str,
    variant_id: Option<&str>,
) -> AppResult<String> {
    transaction::immediate(conn, |tx| record_copy_tx(tx, prompt_id, variant_id))
}

pub(crate) fn record_copy_tx(
    conn: &Connection,
    prompt_id: &str,
    variant_id: Option<&str>,
) -> AppResult<String> {
    ensure_prompt_exists(conn, prompt_id)?;
    let actual_variant_id = match variant_id {
        Some(id) => {
            ensure_variant_belongs_to_prompt(conn, prompt_id, id)?;
            id.to_string()
        }
        None => get_active_primary_variant_id(conn, prompt_id)?,
    };

    let content = conn
        .query_row(
            "SELECT v.content
             FROM variants v
             JOIN prompts p ON p.id = v.prompt_id AND p.deleted_at IS NULL
             WHERE v.id = ?1 AND v.prompt_id = ?2 AND v.deleted_at IS NULL",
            params![actual_variant_id, prompt_id],
            |row| row.get(0),
        )
        .map_err(AppError::from)?;

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO copy_history (id, prompt_id, variant_id, copied_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            uuid::Uuid::new_v4().to_string(),
            prompt_id,
            actual_variant_id,
            now
        ],
    )
    .map_err(AppError::from)?;

    let affected = conn
        .execute(
            "UPDATE prompts
             SET copy_count = copy_count + 1, last_copied_at = ?1, updated_at = ?1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![now, prompt_id],
        )
        .map_err(AppError::from)?;
    transaction::expect_one(affected)?;
    Ok(content)
}

pub fn add_variant(
    conn: &mut Connection,
    prompt_id: &str,
    label: &str,
    content: &str,
) -> AppResult<Variant> {
    transaction::immediate(conn, |tx| add_variant_tx(tx, prompt_id, label, content))
}

pub(crate) fn add_variant_tx(
    conn: &Connection,
    prompt_id: &str,
    label: &str,
    content: &str,
) -> AppResult<Variant> {
    ensure_prompt_exists(conn, prompt_id)?;
    let max_sort = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1)
             FROM variants WHERE prompt_id = ?1 AND deleted_at IS NULL",
            params![prompt_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(AppError::from)?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO variants
            (id, prompt_id, label, content, content_type, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'static', ?5, ?6, ?6)",
        params![id, prompt_id, label, content, max_sort + 1, now],
    )
    .map_err(AppError::from)?;

    let affected = conn
        .execute(
            "UPDATE prompts SET updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![now, prompt_id],
        )
        .map_err(AppError::from)?;
    transaction::expect_one(affected)?;
    update_fts_index_tx(conn, prompt_id)?;

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

pub fn update_variant(
    conn: &mut Connection,
    id: &str,
    content: &str,
    label: Option<&str>,
) -> AppResult<()> {
    transaction::immediate(conn, |tx| update_variant_tx(tx, id, content, label))
}

fn update_variant_tx(
    conn: &Connection,
    id: &str,
    content: &str,
    label: Option<&str>,
) -> AppResult<()> {
    let prompt_id = conn
        .query_row(
            "SELECT v.prompt_id
             FROM variants v
             JOIN prompts p ON p.id = v.prompt_id AND p.deleted_at IS NULL
             WHERE v.id = ?1 AND v.deleted_at IS NULL",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .map_err(AppError::from)?;
    let now = chrono::Utc::now().to_rfc3339();

    let affected = if let Some(label) = label {
        conn.execute(
            "UPDATE variants SET content = ?1, label = ?2, updated_at = ?3
             WHERE id = ?4 AND deleted_at IS NULL",
            params![content, label, now, id],
        )
    } else {
        conn.execute(
            "UPDATE variants SET content = ?1, updated_at = ?2
             WHERE id = ?3 AND deleted_at IS NULL",
            params![content, now, id],
        )
    }
    .map_err(AppError::from)?;
    transaction::expect_one(affected)?;

    let affected = conn
        .execute(
            "UPDATE prompts SET updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![now, prompt_id],
        )
        .map_err(AppError::from)?;
    transaction::expect_one(affected)?;
    update_fts_index_tx(conn, &prompt_id)
}

pub fn delete_variant(conn: &mut Connection, id: &str) -> AppResult<()> {
    transaction::immediate(conn, |tx| delete_variant_tx(tx, id))
}

pub(crate) fn delete_variant_tx(conn: &Connection, id: &str) -> AppResult<()> {
    let (prompt_id, is_primary) = conn
        .query_row(
            "SELECT v.prompt_id, CASE WHEN p.primary_variant_id = v.id THEN 1 ELSE 0 END
             FROM variants v
             JOIN prompts p ON p.id = v.prompt_id
             WHERE v.id = ?1 AND v.deleted_at IS NULL AND p.deleted_at IS NULL",
            params![id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0)),
        )
        .map_err(AppError::from)?;
    let now = chrono::Utc::now().to_rfc3339();

    if is_primary {
        let replacement = next_active_variant_id(conn, &prompt_id, id)?.ok_or_else(|| {
            AppError::Invalid("Cannot delete the only active variant".to_string())
        })?;
        let affected = conn
            .execute(
                "UPDATE prompts SET primary_variant_id = ?1, updated_at = ?2
                 WHERE id = ?3 AND deleted_at IS NULL",
                params![replacement, now, prompt_id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
    }

    let affected = conn
        .execute(
            "UPDATE variants SET deleted_at = ?1, updated_at = ?1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![now, id],
        )
        .map_err(AppError::from)?;
    transaction::expect_one(affected)?;

    if !is_primary {
        let affected = conn
            .execute(
                "UPDATE prompts SET updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
                params![now, prompt_id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
    }

    update_fts_index_tx(conn, &prompt_id)
}

pub(crate) fn update_fts_index_tx(conn: &Connection, prompt_id: &str) -> AppResult<()> {
    let existing_rowid = conn
        .query_row(
            "SELECT rowid FROM fts_mapping WHERE prompt_id = ?1",
            params![prompt_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(AppError::from)?;

    let prompt_data = conn
        .query_row(
            "SELECT p.title, p.description, COALESCE(v.content, '')
             FROM prompts p
             LEFT JOIN variants v
               ON v.id = p.primary_variant_id
              AND v.prompt_id = p.id
              AND v.deleted_at IS NULL
             WHERE p.id = ?1 AND p.deleted_at IS NULL",
            params![prompt_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(AppError::from)?;

    let Some((title, description, content)) = prompt_data else {
        if let Some(rowid) = existing_rowid {
            conn.execute("DELETE FROM prompts_fts WHERE rowid = ?1", params![rowid])
                .map_err(AppError::from)?;
            conn.execute(
                "DELETE FROM fts_mapping WHERE prompt_id = ?1",
                params![prompt_id],
            )
            .map_err(AppError::from)?;
        }
        return Ok(());
    };

    let tags = tag_service::get_tags_for_prompt(conn, prompt_id)?
        .into_iter()
        .map(|tag| tag.name)
        .collect::<Vec<_>>()
        .join(" ");

    let rowid = match existing_rowid {
        Some(rowid) => {
            conn.execute("DELETE FROM prompts_fts WHERE rowid = ?1", params![rowid])
                .map_err(AppError::from)?;
            rowid
        }
        None => {
            conn.execute(
                "INSERT INTO fts_mapping (prompt_id) VALUES (?1)",
                params![prompt_id],
            )
            .map_err(AppError::from)?;
            conn.last_insert_rowid()
        }
    };

    conn.execute(
        "INSERT INTO prompts_fts(rowid, title, description, content, tags)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            rowid,
            title,
            description.as_deref().unwrap_or(""),
            content,
            tags
        ],
    )
    .map_err(AppError::from)?;
    Ok(())
}
