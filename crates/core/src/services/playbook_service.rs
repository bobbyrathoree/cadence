use std::collections::HashSet;

use rusqlite::{params, Connection};

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::patch::PatchField;
use crate::models::playbook::{
    Playbook, PlaybookSession, PlaybookStep, PlaybookStepWithPrompt, PlaybookWithSteps, StepSpec,
    UpdatePlaybookRequest,
};
use crate::services::{prompt_service, transaction};

const ACTIVE_SESSION_CONFLICT: &str = "End the active session to edit this playbook";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybookCountRow {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub step_count: i64,
}

pub fn create_playbook(
    conn: &mut Db,
    title: &str,
    description: Option<&str>,
) -> AppResult<Playbook> {
    transaction::immediate(conn, |tx| create_playbook_tx(tx, title, description))
}

pub(crate) fn create_playbook_tx(
    conn: &Connection,
    title: &str,
    description: Option<&str>,
) -> AppResult<Playbook> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO playbooks (id, title, description, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, title, description, now],
    )
    .map_err(AppError::from)?;
    Ok(Playbook {
        id,
        title: title.to_string(),
        description: description.map(str::to_string),
        created_at: Some(now.clone()),
        updated_at: Some(now),
    })
}

pub fn get_playbook(conn: &Connection, id: &str) -> AppResult<PlaybookWithSteps> {
    let playbook = conn
        .query_row(
            "SELECT id, title, description, created_at, updated_at
             FROM playbooks WHERE id = ?1",
            params![id],
            |row| {
                Ok(Playbook {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .map_err(AppError::from)?;

    let steps = {
        let mut stmt = conn
            .prepare(
                "SELECT id, playbook_id, prompt_id, position, step_type, instructions,
                        choice_prompt_ids
                 FROM playbook_steps
                 WHERE playbook_id = ?1
                 ORDER BY position",
            )
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map(params![id], map_step)
            .map_err(AppError::from)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(AppError::from)?
    };

    let hydrated_steps = steps
        .into_iter()
        .map(|step| hydrate_step(conn, step))
        .collect::<AppResult<Vec<_>>>()?;

    Ok(PlaybookWithSteps {
        playbook,
        steps: hydrated_steps,
    })
}

fn optional_prompt(
    conn: &Connection,
    prompt_id: &str,
) -> AppResult<Option<crate::models::prompt::PromptWithVariants>> {
    match prompt_service::get_prompt_by_id(conn, prompt_id) {
        Ok(prompt) => Ok(Some(prompt)),
        Err(AppError::NotFound) => Ok(None),
        Err(error) => Err(error),
    }
}

fn hydrate_step(conn: &Connection, step: PlaybookStep) -> AppResult<PlaybookStepWithPrompt> {
    let prompt = match step.prompt_id.as_deref() {
        Some(prompt_id) => optional_prompt(conn, prompt_id)?,
        None => None,
    };
    let choice_prompts = step
        .choice_prompt_ids
        .iter()
        .map(|prompt_id| optional_prompt(conn, prompt_id))
        .filter_map(|result| match result {
            Ok(Some(prompt)) => Some(Ok(prompt)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<AppResult<Vec<_>>>()?;

    Ok(PlaybookStepWithPrompt {
        step,
        prompt,
        choice_prompts,
    })
}

pub fn list_playbooks(conn: &Connection) -> AppResult<Vec<Playbook>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, description, created_at, updated_at
             FROM playbooks
             ORDER BY updated_at DESC",
        )
        .map_err(AppError::from)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Playbook {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(AppError::from)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)
}

pub fn list_playbooks_with_counts(conn: &Connection) -> AppResult<Vec<PlaybookCountRow>> {
    query_playbooks_with_counts(
        conn,
        "SELECT p.id, p.title, p.description, COUNT(s.id)
         FROM playbooks p
         LEFT JOIN playbook_steps s ON s.playbook_id = p.id
         GROUP BY p.id, p.title, p.description
         ORDER BY p.title ASC, p.id ASC",
        [],
    )
}

pub fn find_playbooks_by_exact_title(
    conn: &Connection,
    title: &str,
) -> AppResult<Vec<PlaybookCountRow>> {
    query_playbooks_with_counts(
        conn,
        "SELECT p.id, p.title, p.description, COUNT(s.id)
         FROM playbooks p
         LEFT JOIN playbook_steps s ON s.playbook_id = p.id
         WHERE p.title = ?1
         GROUP BY p.id, p.title, p.description
         ORDER BY p.title ASC, p.id ASC",
        params![title],
    )
}

pub fn complete_playbook_ids(conn: &Connection, prefix: &str, cap: u32) -> AppResult<Vec<String>> {
    let escaped = prompt_service::escape_like(prefix);
    let mut stmt = conn
        .prepare(
            "SELECT id FROM playbooks
             WHERE lower(title) LIKE lower(?1) || '%' ESCAPE '\\'
                OR lower(id) LIKE lower(?1) || '%' ESCAPE '\\'
             ORDER BY title ASC, id ASC
             LIMIT ?2",
        )
        .map_err(AppError::from)?;
    let rows = stmt
        .query_map(params![escaped, i64::from(cap) + 1], |row| {
            row.get::<_, String>(0)
        })
        .map_err(AppError::from)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)
}

fn query_playbooks_with_counts<P>(
    conn: &Connection,
    sql: &str,
    parameters: P,
) -> AppResult<Vec<PlaybookCountRow>>
where
    P: rusqlite::Params,
{
    let mut stmt = conn.prepare(sql).map_err(AppError::from)?;
    let rows = stmt
        .query_map(parameters, |row| {
            Ok(PlaybookCountRow {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                step_count: row.get(3)?,
            })
        })
        .map_err(AppError::from)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)
}

pub fn update_playbook(
    conn: &mut Db,
    id: &str,
    request: UpdatePlaybookRequest,
) -> AppResult<Playbook> {
    transaction::immediate(conn, |tx| update_playbook_tx(tx, id, request.clone()))
}

pub(crate) fn update_playbook_tx(
    conn: &Connection,
    id: &str,
    request: UpdatePlaybookRequest,
) -> AppResult<Playbook> {
    ensure_playbook_exists(conn, id)?;
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(ref title) = request.title {
        let affected = conn
            .execute(
                "UPDATE playbooks SET title = ?1, updated_at = ?2 WHERE id = ?3",
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
                    "UPDATE playbooks SET description = NULL, updated_at = ?1 WHERE id = ?2",
                    params![now, id],
                )
                .map_err(AppError::from)?;
            transaction::expect_one(affected)?;
        }
        PatchField::Set(description) => {
            let affected = conn
                .execute(
                    "UPDATE playbooks SET description = ?1, updated_at = ?2 WHERE id = ?3",
                    params![description, now, id],
                )
                .map_err(AppError::from)?;
            transaction::expect_one(affected)?;
        }
    }
    if request.title.is_none() && request.description.is_keep() {
        let affected = conn
            .execute(
                "UPDATE playbooks SET updated_at = ?1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
    }
    get_playbook_record(conn, id)
}

pub fn delete_playbook(conn: &mut Db, id: &str) -> AppResult<()> {
    transaction::immediate(conn, |tx| delete_playbook_tx(tx, id))
}

fn delete_playbook_tx(conn: &Connection, id: &str) -> AppResult<()> {
    ensure_playbook_exists(conn, id)?;
    ensure_no_active_session(conn, id)?;
    let affected = conn
        .execute("DELETE FROM playbooks WHERE id = ?1", params![id])
        .map_err(AppError::from)?;
    transaction::expect_one(affected)
}

pub fn add_step(
    conn: &mut Db,
    playbook_id: &str,
    spec: StepSpec,
) -> AppResult<PlaybookStepWithPrompt> {
    transaction::immediate(conn, |tx| add_step_tx(tx, playbook_id, spec.clone()))
}

pub(crate) fn add_step_tx(
    conn: &Connection,
    playbook_id: &str,
    spec: StepSpec,
) -> AppResult<PlaybookStepWithPrompt> {
    ensure_playbook_exists(conn, playbook_id)?;
    ensure_no_active_session(conn, playbook_id)?;
    validate_step(
        conn,
        &spec.step_type,
        spec.prompt_id.as_deref(),
        &spec.choice_prompt_ids,
    )?;
    let position = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1
             FROM playbook_steps WHERE playbook_id = ?1",
            params![playbook_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(AppError::from)?;
    let id = uuid::Uuid::new_v4().to_string();
    let stored_choice_ids = join_choice_ids(&spec.choice_prompt_ids);

    conn.execute(
        "INSERT INTO playbook_steps
            (id, playbook_id, prompt_id, position, step_type, instructions, choice_prompt_ids)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            playbook_id,
            spec.prompt_id,
            position,
            spec.step_type,
            spec.instructions,
            stored_choice_ids
        ],
    )
    .map_err(AppError::from)?;
    touch_playbook(conn, playbook_id)?;

    hydrate_step(
        conn,
        PlaybookStep {
            id,
            playbook_id: playbook_id.to_string(),
            prompt_id: spec.prompt_id,
            position,
            step_type: spec.step_type,
            instructions: spec.instructions,
            choice_prompt_ids: spec.choice_prompt_ids,
        },
    )
}

pub fn update_step(
    conn: &mut Db,
    playbook_id: &str,
    step_id: &str,
    spec: StepSpec,
) -> AppResult<PlaybookStepWithPrompt> {
    transaction::immediate(conn, |tx| {
        update_step_tx(tx, playbook_id, step_id, spec.clone())
    })
}

fn update_step_tx(
    conn: &Connection,
    playbook_id: &str,
    step_id: &str,
    spec: StepSpec,
) -> AppResult<PlaybookStepWithPrompt> {
    let position = ensure_step_owned(conn, playbook_id, step_id)?;
    ensure_no_active_session(conn, playbook_id)?;
    validate_step(
        conn,
        &spec.step_type,
        spec.prompt_id.as_deref(),
        &spec.choice_prompt_ids,
    )?;
    let stored_choice_ids = join_choice_ids(&spec.choice_prompt_ids);
    let affected = conn
        .execute(
            "UPDATE playbook_steps
             SET prompt_id = ?1, step_type = ?2, instructions = ?3,
                 choice_prompt_ids = ?4
             WHERE id = ?5 AND playbook_id = ?6",
            params![
                spec.prompt_id,
                spec.step_type,
                spec.instructions,
                stored_choice_ids,
                step_id,
                playbook_id
            ],
        )
        .map_err(AppError::from)?;
    transaction::expect_one(affected)?;
    touch_playbook(conn, playbook_id)?;
    hydrate_step(
        conn,
        PlaybookStep {
            id: step_id.to_string(),
            playbook_id: playbook_id.to_string(),
            prompt_id: spec.prompt_id,
            position,
            step_type: spec.step_type,
            instructions: spec.instructions,
            choice_prompt_ids: spec.choice_prompt_ids,
        },
    )
}

pub fn remove_step(conn: &mut Db, playbook_id: &str, step_id: &str) -> AppResult<()> {
    transaction::immediate(conn, |tx| remove_step_tx(tx, playbook_id, step_id))
}

pub(crate) fn remove_step_tx(conn: &Connection, playbook_id: &str, step_id: &str) -> AppResult<()> {
    ensure_step_owned(conn, playbook_id, step_id)?;
    ensure_no_active_session(conn, playbook_id)?;
    let affected = conn
        .execute(
            "DELETE FROM playbook_steps WHERE id = ?1 AND playbook_id = ?2",
            params![step_id, playbook_id],
        )
        .map_err(AppError::from)?;
    transaction::expect_one(affected)?;
    compact_step_positions(conn, playbook_id)?;
    touch_playbook(conn, playbook_id)
}

pub fn reorder_steps(
    conn: &mut Db,
    playbook_id: &str,
    ordered_step_ids: &[String],
) -> AppResult<()> {
    transaction::immediate(conn, |tx| {
        reorder_steps_tx(tx, playbook_id, ordered_step_ids)
    })
}

fn reorder_steps_tx(
    conn: &Connection,
    playbook_id: &str,
    ordered_step_ids: &[String],
) -> AppResult<()> {
    ensure_playbook_exists(conn, playbook_id)?;
    ensure_no_active_session(conn, playbook_id)?;
    let existing = {
        let mut stmt = conn
            .prepare("SELECT id FROM playbook_steps WHERE playbook_id = ?1")
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map(params![playbook_id], |row| row.get::<_, String>(0))
            .map_err(AppError::from)?;
        rows.collect::<rusqlite::Result<HashSet<_>>>()
            .map_err(AppError::from)?
    };
    let requested = ordered_step_ids.iter().cloned().collect::<HashSet<_>>();
    if existing.len() != ordered_step_ids.len() || existing != requested {
        return Err(AppError::invalid(
            "Step order must contain every playbook step exactly once",
        ));
    }

    conn.execute(
        "UPDATE playbook_steps SET position = -position - 1 WHERE playbook_id = ?1",
        params![playbook_id],
    )
    .map_err(AppError::from)?;
    for (position, step_id) in ordered_step_ids.iter().enumerate() {
        let affected = conn
            .execute(
                "UPDATE playbook_steps SET position = ?1
                 WHERE id = ?2 AND playbook_id = ?3",
                params![position as i64, step_id, playbook_id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
    }
    touch_playbook(conn, playbook_id)
}

fn compact_step_positions(conn: &Connection, playbook_id: &str) -> AppResult<()> {
    let step_ids = {
        let mut stmt = conn
            .prepare(
                "SELECT id FROM playbook_steps
                 WHERE playbook_id = ?1 ORDER BY position, id",
            )
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map(params![playbook_id], |row| row.get::<_, String>(0))
            .map_err(AppError::from)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(AppError::from)?
    };
    conn.execute(
        "UPDATE playbook_steps SET position = -position - 1 WHERE playbook_id = ?1",
        params![playbook_id],
    )
    .map_err(AppError::from)?;
    for (position, step_id) in step_ids.iter().enumerate() {
        let affected = conn
            .execute(
                "UPDATE playbook_steps SET position = ?1 WHERE id = ?2",
                params![position as i64, step_id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
    }
    Ok(())
}

pub fn get_session(conn: &Connection) -> AppResult<PlaybookSession> {
    conn.query_row(
        "SELECT id, active_playbook_id, current_step, started_at
         FROM playbook_sessions WHERE id = 1",
        [],
        |row| {
            Ok(PlaybookSession {
                id: row.get(0)?,
                active_playbook_id: row.get(1)?,
                current_step: row.get(2)?,
                started_at: row.get(3)?,
            })
        },
    )
    .map_err(AppError::from)
}

pub fn start_session(conn: &mut Db, playbook_id: &str) -> AppResult<PlaybookSession> {
    transaction::immediate(conn, |tx| start_session_tx(tx, playbook_id))
}

pub(crate) fn start_session_tx(conn: &Connection, playbook_id: &str) -> AppResult<PlaybookSession> {
    ensure_playbook_exists(conn, playbook_id)?;
    let affected = conn
        .execute(
            "UPDATE playbook_sessions
             SET active_playbook_id = ?1, current_step = 0, started_at = ?2
             WHERE id = 1",
            params![playbook_id, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(AppError::from)?;
    transaction::expect_one(affected)?;
    get_session(conn)
}

pub fn advance_step(conn: &mut Db) -> AppResult<PlaybookSession> {
    transaction::immediate(conn, advance_step_tx)
}

pub(crate) fn advance_step_tx(conn: &Connection) -> AppResult<PlaybookSession> {
    let affected = conn
        .execute(
            "UPDATE playbook_sessions
             SET current_step = current_step + 1
             WHERE id = 1 AND active_playbook_id IS NOT NULL",
            [],
        )
        .map_err(AppError::from)?;
    transaction::expect_one(affected)?;
    get_session(conn)
}

pub fn end_session(conn: &mut Db) -> AppResult<()> {
    transaction::immediate(conn, end_session_tx)
}

pub(crate) fn end_session_tx(conn: &Connection) -> AppResult<()> {
    let affected = conn
        .execute(
            "UPDATE playbook_sessions
             SET active_playbook_id = NULL, current_step = 0, started_at = NULL
             WHERE id = 1",
            [],
        )
        .map_err(AppError::from)?;
    transaction::expect_one(affected)
}

fn ensure_playbook_exists(conn: &Connection, playbook_id: &str) -> AppResult<()> {
    conn.query_row(
        "SELECT 1 FROM playbooks WHERE id = ?1",
        params![playbook_id],
        |_| Ok(()),
    )
    .map_err(AppError::from)
}

fn get_playbook_record(conn: &Connection, playbook_id: &str) -> AppResult<Playbook> {
    conn.query_row(
        "SELECT id, title, description, created_at, updated_at
         FROM playbooks WHERE id = ?1",
        params![playbook_id],
        |row| {
            Ok(Playbook {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        },
    )
    .map_err(AppError::from)
}

fn ensure_step_owned(conn: &Connection, playbook_id: &str, step_id: &str) -> AppResult<i64> {
    conn.query_row(
        "SELECT position FROM playbook_steps WHERE id = ?1 AND playbook_id = ?2",
        params![step_id, playbook_id],
        |row| row.get(0),
    )
    .map_err(AppError::from)
}

fn ensure_no_active_session(conn: &Connection, playbook_id: &str) -> AppResult<()> {
    let is_active = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM playbook_sessions
                WHERE id = 1 AND active_playbook_id = ?1
             )",
            params![playbook_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(AppError::from)?;
    if is_active {
        return Err(AppError::Conflict(ACTIVE_SESSION_CONFLICT.to_string()));
    }
    Ok(())
}

pub fn validate_step(
    conn: &Connection,
    step_type: &str,
    prompt_id: Option<&str>,
    choice_prompt_ids: &[String],
) -> AppResult<()> {
    match step_type {
        "single" => {
            if !choice_prompt_ids.is_empty() {
                return Err(AppError::invalid(
                    "Single steps cannot contain choice prompts",
                ));
            }
            let prompt_id =
                prompt_id.ok_or_else(|| AppError::invalid("Single steps require a prompt"))?;
            ensure_active_prompt(conn, prompt_id)
        }
        "choice" => {
            if prompt_id.is_some() {
                return Err(AppError::invalid(
                    "Choice steps cannot contain a single prompt",
                ));
            }
            let distinct = choice_prompt_ids.iter().collect::<HashSet<_>>();
            if choice_prompt_ids.len() < 2 || distinct.len() != choice_prompt_ids.len() {
                return Err(AppError::invalid(
                    "Choice steps require at least two distinct prompts",
                ));
            }
            for choice_prompt_id in choice_prompt_ids {
                ensure_active_prompt(conn, choice_prompt_id)?;
            }
            Ok(())
        }
        _ => Err(AppError::invalid(
            "step_type must be either single or choice",
        )),
    }
}

fn ensure_active_prompt(conn: &Connection, prompt_id: &str) -> AppResult<()> {
    let exists = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM prompts WHERE id = ?1 AND deleted_at IS NULL
             )",
            params![prompt_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(AppError::from)?;
    if !exists {
        return Err(AppError::invalid("Step prompts must exist and be active"));
    }
    Ok(())
}

fn join_choice_ids(choice_prompt_ids: &[String]) -> Option<String> {
    (!choice_prompt_ids.is_empty()).then(|| choice_prompt_ids.join(","))
}

fn split_choice_ids(choice_prompt_ids: Option<String>) -> Vec<String> {
    choice_prompt_ids
        .map(|ids| {
            ids.split(',')
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn touch_playbook(conn: &Connection, playbook_id: &str) -> AppResult<()> {
    let affected = conn
        .execute(
            "UPDATE playbooks SET updated_at = ?1 WHERE id = ?2",
            params![chrono::Utc::now().to_rfc3339(), playbook_id],
        )
        .map_err(AppError::from)?;
    transaction::expect_one(affected)
}

fn map_step(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlaybookStep> {
    let choice_prompt_ids = row.get::<_, Option<String>>(6)?;
    Ok(PlaybookStep {
        id: row.get(0)?,
        playbook_id: row.get(1)?,
        prompt_id: row.get(2)?,
        position: row.get(3)?,
        step_type: row.get(4)?,
        instructions: row.get(5)?,
        choice_prompt_ids: split_choice_ids(choice_prompt_ids),
    })
}
