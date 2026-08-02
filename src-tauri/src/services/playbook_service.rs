use std::collections::HashSet;

use rusqlite::{params, Connection};

use crate::error::{AppError, AppResult};
use crate::models::playbook::{
    Playbook, PlaybookSession, PlaybookStep, PlaybookStepWithPrompt, PlaybookWithSteps,
};
use crate::services::{prompt_service, transaction};

pub fn create_playbook(
    conn: &mut Connection,
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

    let mut hydrated_steps = Vec::with_capacity(steps.len());
    for step in steps {
        let prompt = match step.prompt_id.as_deref() {
            Some(prompt_id) => optional_prompt(conn, prompt_id)?,
            None => None,
        };

        let mut choice_prompts = Vec::new();
        if let Some(choice_ids) = step.choice_prompt_ids.as_deref() {
            for prompt_id in choice_ids
                .split(',')
                .map(str::trim)
                .filter(|id| !id.is_empty())
            {
                if let Some(prompt) = optional_prompt(conn, prompt_id)? {
                    choice_prompts.push(prompt);
                }
            }
        }

        hydrated_steps.push(PlaybookStepWithPrompt {
            step,
            prompt,
            choice_prompts: (!choice_prompts.is_empty()).then_some(choice_prompts),
        });
    }

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

pub fn update_playbook(
    conn: &mut Connection,
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
) -> AppResult<()> {
    transaction::immediate(conn, |tx| update_playbook_tx(tx, id, title, description))
}

pub(crate) fn update_playbook_tx(
    conn: &Connection,
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
) -> AppResult<()> {
    ensure_playbook_exists(conn, id)?;
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(title) = title {
        let affected = conn
            .execute(
                "UPDATE playbooks SET title = ?1, updated_at = ?2 WHERE id = ?3",
                params![title, now, id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
    }
    if let Some(description) = description {
        let affected = conn
            .execute(
                "UPDATE playbooks SET description = ?1, updated_at = ?2 WHERE id = ?3",
                params![description, now, id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
    }
    if title.is_none() && description.is_none() {
        let affected = conn
            .execute(
                "UPDATE playbooks SET updated_at = ?1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
    }
    Ok(())
}

pub fn delete_playbook(conn: &mut Connection, id: &str) -> AppResult<()> {
    transaction::immediate(conn, |tx| {
        let affected = tx
            .execute("DELETE FROM playbooks WHERE id = ?1", params![id])
            .map_err(AppError::from)?;
        transaction::expect_one(affected)
    })
}

#[allow(clippy::too_many_arguments)]
pub fn add_step(
    conn: &mut Connection,
    playbook_id: &str,
    prompt_id: Option<&str>,
    step_type: &str,
    instructions: Option<&str>,
    choice_prompt_ids: Option<Vec<String>>,
) -> AppResult<PlaybookStep> {
    transaction::immediate(conn, move |tx| {
        add_step_tx(
            tx,
            playbook_id,
            prompt_id,
            step_type,
            instructions,
            choice_prompt_ids,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn add_step_tx(
    conn: &Connection,
    playbook_id: &str,
    prompt_id: Option<&str>,
    step_type: &str,
    instructions: Option<&str>,
    choice_prompt_ids: Option<Vec<String>>,
) -> AppResult<PlaybookStep> {
    ensure_playbook_exists(conn, playbook_id)?;
    let position = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1
             FROM playbook_steps WHERE playbook_id = ?1",
            params![playbook_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(AppError::from)?;
    let id = uuid::Uuid::new_v4().to_string();
    let choice_ids = choice_prompt_ids.map(|ids| ids.join(","));

    conn.execute(
        "INSERT INTO playbook_steps
            (id, playbook_id, prompt_id, position, step_type, instructions, choice_prompt_ids)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            playbook_id,
            prompt_id,
            position,
            step_type,
            instructions,
            choice_ids
        ],
    )
    .map_err(AppError::from)?;
    touch_playbook(conn, playbook_id)?;

    Ok(PlaybookStep {
        id,
        playbook_id: playbook_id.to_string(),
        prompt_id: prompt_id.map(str::to_string),
        position,
        step_type: Some(step_type.to_string()),
        instructions: instructions.map(str::to_string),
        choice_prompt_ids: choice_ids,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn update_step(
    conn: &mut Connection,
    playbook_id: &str,
    step_id: &str,
    prompt_id: Option<&str>,
    step_type: &str,
    instructions: Option<&str>,
    choice_prompt_ids: Option<Vec<String>>,
) -> AppResult<PlaybookStep> {
    transaction::immediate(conn, move |tx| {
        let position = tx
            .query_row(
                "SELECT position FROM playbook_steps
                 WHERE id = ?1 AND playbook_id = ?2",
                params![step_id, playbook_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(AppError::from)?;
        let choice_ids = choice_prompt_ids.map(|ids| ids.join(","));
        let affected = tx
            .execute(
                "UPDATE playbook_steps
                 SET prompt_id = ?1, step_type = ?2, instructions = ?3,
                     choice_prompt_ids = ?4
                 WHERE id = ?5 AND playbook_id = ?6",
                params![
                    prompt_id,
                    step_type,
                    instructions,
                    choice_ids,
                    step_id,
                    playbook_id
                ],
            )
            .map_err(AppError::from)?;
        transaction::expect_one(affected)?;
        touch_playbook(tx, playbook_id)?;
        Ok(PlaybookStep {
            id: step_id.to_string(),
            playbook_id: playbook_id.to_string(),
            prompt_id: prompt_id.map(str::to_string),
            position,
            step_type: Some(step_type.to_string()),
            instructions: instructions.map(str::to_string),
            choice_prompt_ids: choice_ids,
        })
    })
}

pub fn remove_step(conn: &mut Connection, step_id: &str) -> AppResult<()> {
    transaction::immediate(conn, |tx| remove_step_tx(tx, step_id))
}

pub(crate) fn remove_step_tx(conn: &Connection, step_id: &str) -> AppResult<()> {
    let playbook_id = conn
        .query_row(
            "SELECT playbook_id FROM playbook_steps WHERE id = ?1",
            params![step_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(AppError::from)?;
    let affected = conn
        .execute("DELETE FROM playbook_steps WHERE id = ?1", params![step_id])
        .map_err(AppError::from)?;
    transaction::expect_one(affected)?;
    compact_step_positions(conn, &playbook_id)?;
    touch_playbook(conn, &playbook_id)
}

pub fn reorder_steps(
    conn: &mut Connection,
    playbook_id: &str,
    ordered_step_ids: &[String],
) -> AppResult<()> {
    transaction::immediate(conn, |tx| {
        let existing = {
            let mut stmt = tx
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

        tx.execute(
            "UPDATE playbook_steps SET position = -position - 1 WHERE playbook_id = ?1",
            params![playbook_id],
        )
        .map_err(AppError::from)?;
        for (position, step_id) in ordered_step_ids.iter().enumerate() {
            let affected = tx
                .execute(
                    "UPDATE playbook_steps SET position = ?1
                     WHERE id = ?2 AND playbook_id = ?3",
                    params![position as i64, step_id, playbook_id],
                )
                .map_err(AppError::from)?;
            transaction::expect_one(affected)?;
        }
        touch_playbook(tx, playbook_id)
    })
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

pub fn start_session(conn: &mut Connection, playbook_id: &str) -> AppResult<PlaybookSession> {
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

pub fn advance_step(conn: &mut Connection) -> AppResult<PlaybookSession> {
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

pub fn end_session(conn: &mut Connection) -> AppResult<()> {
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
    Ok(PlaybookStep {
        id: row.get(0)?,
        playbook_id: row.get(1)?,
        prompt_id: row.get(2)?,
        position: row.get(3)?,
        step_type: row.get(4)?,
        instructions: row.get(5)?,
        choice_prompt_ids: row.get(6)?,
    })
}
