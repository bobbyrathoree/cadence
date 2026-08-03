use cadence_core::db::Db;
use cadence_core::error::{AppError, AppResult};
use cadence_core::models::playbook::PlaybookWithSteps;
use cadence_core::models::prompt::{Prompt, PromptWithVariants};
use cadence_core::services::{playbook_service, prompt_service};

use crate::server::CadenceMcp;

pub mod production;
#[cfg_attr(not(feature = "test-support"), allow(dead_code))]
pub mod reference;

#[cfg_attr(not(feature = "test-support"), allow(dead_code))]
pub(crate) fn resolve_prompt(db: &mut Db, id_or_title: &str) -> AppResult<Prompt> {
    resolve_prompt_detail(db, id_or_title).map(|prompt| prompt.prompt)
}

pub(crate) fn resolve_prompt_detail(
    db: &mut Db,
    id_or_title: &str,
) -> AppResult<PromptWithVariants> {
    if uuid::Uuid::parse_str(id_or_title).is_ok() {
        match prompt_service::get_prompt_by_id(&db.conn, id_or_title) {
            Ok(prompt) => return Ok(prompt),
            Err(AppError::NotFound) => {}
            Err(error) => return Err(error),
        }
    }

    let candidates = prompt_service::find_prompts_by_exact_title(&db.conn, id_or_title)?;
    match candidates.as_slice() {
        [] => Err(AppError::invalid("not found")),
        [candidate] => prompt_service::get_prompt_by_id(&db.conn, &candidate.id),
        _ => Err(ambiguous_error(
            candidates
                .into_iter()
                .map(|candidate| (candidate.id, candidate.title)),
        )),
    }
}

pub(crate) fn resolve_playbook(db: &mut Db, id_or_title: &str) -> AppResult<PlaybookWithSteps> {
    if uuid::Uuid::parse_str(id_or_title).is_ok() {
        match playbook_service::get_playbook(&db.conn, id_or_title) {
            Ok(playbook) => return Ok(playbook),
            Err(AppError::NotFound) => {}
            Err(error) => return Err(error),
        }
    }

    let candidates = playbook_service::find_playbooks_by_exact_title(&db.conn, id_or_title)?;
    match candidates.as_slice() {
        [] => Err(AppError::invalid("not found")),
        [candidate] => playbook_service::get_playbook(&db.conn, &candidate.id),
        _ => Err(ambiguous_error(
            candidates
                .into_iter()
                .map(|candidate| (candidate.id, candidate.title)),
        )),
    }
}

fn ambiguous_error(candidates: impl Iterator<Item = (String, String)>) -> AppError {
    let candidates = candidates
        .map(|(id, title)| serde_json::json!({ "id": id, "title": title }))
        .collect::<Vec<_>>();
    AppError::invalid(format!(
        "ambiguous title; candidates: {}; call again with an id",
        serde_json::Value::Array(candidates)
    ))
}
