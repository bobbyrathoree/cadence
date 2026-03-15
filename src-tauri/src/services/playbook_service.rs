use rusqlite::{params, Connection};

use crate::models::playbook::{
    Playbook, PlaybookSession, PlaybookStep, PlaybookStepWithPrompt, PlaybookWithSteps,
};
use crate::services::prompt_service;

/// Create a new playbook.
pub fn create_playbook(
    conn: &Connection,
    title: &str,
    description: Option<&str>,
) -> rusqlite::Result<Playbook> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO playbooks (id, title, description, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, title, description, now, now],
    )?;

    Ok(Playbook {
        id,
        title: title.to_string(),
        description: description.map(|s| s.to_string()),
        created_at: Some(now.clone()),
        updated_at: Some(now),
    })
}

/// Get a playbook by ID with all steps hydrated with prompt data.
pub fn get_playbook(conn: &Connection, id: &str) -> rusqlite::Result<PlaybookWithSteps> {
    let playbook = conn.query_row(
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
    )?;

    let mut stmt = conn.prepare(
        "SELECT id, playbook_id, prompt_id, position, step_type, instructions, choice_prompt_ids
         FROM playbook_steps
         WHERE playbook_id = ?1
         ORDER BY position",
    )?;

    let steps = stmt
        .query_map(params![id], |row| {
            Ok(PlaybookStep {
                id: row.get(0)?,
                playbook_id: row.get(1)?,
                prompt_id: row.get(2)?,
                position: row.get(3)?,
                step_type: row.get(4)?,
                instructions: row.get(5)?,
                choice_prompt_ids: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // Hydrate each step with prompt data
    let mut hydrated_steps = Vec::new();
    for step in steps {
        // Load the main prompt if present
        let prompt = match &step.prompt_id {
            Some(pid) => prompt_service::get_prompt_by_id(conn, pid).ok(),
            None => None,
        };

        // Load choice prompts if present
        let choice_prompts = match &step.choice_prompt_ids {
            Some(ids_str) if !ids_str.is_empty() => {
                let ids: Vec<&str> = ids_str.split(',').map(|s| s.trim()).collect();
                let mut prompts = Vec::new();
                for cid in ids {
                    if !cid.is_empty() {
                        if let Ok(p) = prompt_service::get_prompt_by_id(conn, cid) {
                            prompts.push(p);
                        }
                    }
                }
                if prompts.is_empty() {
                    None
                } else {
                    Some(prompts)
                }
            }
            _ => None,
        };

        hydrated_steps.push(PlaybookStepWithPrompt {
            step,
            prompt,
            choice_prompts,
        });
    }

    Ok(PlaybookWithSteps {
        playbook,
        steps: hydrated_steps,
    })
}

/// List all playbooks.
pub fn list_playbooks(conn: &Connection) -> rusqlite::Result<Vec<Playbook>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, description, created_at, updated_at
         FROM playbooks
         ORDER BY updated_at DESC",
    )?;

    let playbooks = stmt
        .query_map([], |row| {
            Ok(Playbook {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(playbooks)
}

/// Update a playbook. Only non-None fields are updated.
pub fn update_playbook(
    conn: &Connection,
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(t) = title {
        conn.execute(
            "UPDATE playbooks SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![t, now, id],
        )?;
    }
    if let Some(d) = description {
        conn.execute(
            "UPDATE playbooks SET description = ?1, updated_at = ?2 WHERE id = ?3",
            params![d, now, id],
        )?;
    }

    // If no fields were set, still update updated_at
    if title.is_none() && description.is_none() {
        conn.execute(
            "UPDATE playbooks SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
    }

    Ok(())
}

/// Hard delete a playbook and all its steps (via CASCADE).
pub fn delete_playbook(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM playbooks WHERE id = ?1", params![id])?;
    Ok(())
}

/// Add a step to a playbook at the next position.
pub fn add_step(
    conn: &Connection,
    playbook_id: &str,
    prompt_id: Option<&str>,
    step_type: &str,
    instructions: Option<&str>,
    choice_prompt_ids: Option<Vec<String>>,
) -> rusqlite::Result<PlaybookStep> {
    let id = uuid::Uuid::new_v4().to_string();

    // Determine next position
    let max_position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) FROM playbook_steps WHERE playbook_id = ?1",
            params![playbook_id],
            |row| row.get(0),
        )?;
    let position = max_position + 1;

    let choice_ids_str = choice_prompt_ids
        .as_ref()
        .map(|ids| ids.join(","));

    conn.execute(
        "INSERT INTO playbook_steps (id, playbook_id, prompt_id, position, step_type, instructions, choice_prompt_ids)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, playbook_id, prompt_id, position, step_type, instructions, choice_ids_str],
    )?;

    // Update playbook's updated_at
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE playbooks SET updated_at = ?1 WHERE id = ?2",
        params![now, playbook_id],
    )?;

    Ok(PlaybookStep {
        id,
        playbook_id: playbook_id.to_string(),
        prompt_id: prompt_id.map(|s| s.to_string()),
        position,
        step_type: Some(step_type.to_string()),
        instructions: instructions.map(|s| s.to_string()),
        choice_prompt_ids: choice_ids_str,
    })
}

/// Remove a step and reorder the remaining positions.
pub fn remove_step(conn: &Connection, step_id: &str) -> rusqlite::Result<()> {
    // Get playbook_id and position of the step being removed
    let (playbook_id, position): (String, i64) = conn.query_row(
        "SELECT playbook_id, position FROM playbook_steps WHERE id = ?1",
        params![step_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    // Delete the step
    conn.execute("DELETE FROM playbook_steps WHERE id = ?1", params![step_id])?;

    // Reorder remaining steps: decrement position for all steps after the removed one
    conn.execute(
        "UPDATE playbook_steps SET position = position - 1
         WHERE playbook_id = ?1 AND position > ?2",
        params![playbook_id, position],
    )?;

    // Update playbook's updated_at
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE playbooks SET updated_at = ?1 WHERE id = ?2",
        params![now, playbook_id],
    )?;

    Ok(())
}

/// Ensure the singleton session row exists and return it.
fn ensure_session_row(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO playbook_sessions (id, active_playbook_id, current_step, started_at)
         VALUES (1, NULL, 0, NULL)",
        [],
    )?;
    Ok(())
}

/// Get the current playbook session state.
pub fn get_session(conn: &Connection) -> rusqlite::Result<PlaybookSession> {
    ensure_session_row(conn)?;
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
}

/// Start a playbook session by setting the active playbook and resetting the step counter.
pub fn start_session(conn: &Connection, playbook_id: &str) -> rusqlite::Result<PlaybookSession> {
    ensure_session_row(conn)?;
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE playbook_sessions SET active_playbook_id = ?1, current_step = 0, started_at = ?2
         WHERE id = 1",
        params![playbook_id, now],
    )?;

    get_session(conn)
}

/// Advance to the next step in the active playbook session.
pub fn advance_step(conn: &Connection) -> rusqlite::Result<PlaybookSession> {
    ensure_session_row(conn)?;

    conn.execute(
        "UPDATE playbook_sessions SET current_step = current_step + 1 WHERE id = 1",
        [],
    )?;

    get_session(conn)
}

/// End the current playbook session by clearing the active playbook.
pub fn end_session(conn: &Connection) -> rusqlite::Result<()> {
    ensure_session_row(conn)?;

    conn.execute(
        "UPDATE playbook_sessions SET active_playbook_id = NULL, current_step = 0, started_at = NULL
         WHERE id = 1",
        [],
    )?;

    Ok(())
}
