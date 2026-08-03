use cadence_core::db::{schema, Db, Health};
use cadence_core::error::AppError;
use cadence_core::models::patch::PatchField;
use cadence_core::models::playbook::{StepSpec, UpdatePlaybookRequest};
use cadence_core::models::prompt::{CreatePromptRequest, UpdatePromptRequest};
use cadence_core::services::{playbook_service, prompt_service};
use rusqlite::Connection;

fn setup_db() -> Db {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    schema::create_tables(&conn).unwrap();
    Db {
        conn,
        health: Health::exit_process(1),
    }
}

fn create_prompt(conn: &mut Db, title: &str) -> String {
    prompt_service::create_prompt(
        conn,
        CreatePromptRequest {
            title: title.to_string(),
            description: None,
            content: format!("{title} content"),
            variant_label: None,
            tags: Vec::new(),
            is_favorite: false,
        },
    )
    .unwrap()
    .prompt
    .id
}

fn single(prompt_id: &str) -> StepSpec {
    StepSpec {
        step_type: "single".to_string(),
        prompt_id: Some(prompt_id.to_string()),
        choice_prompt_ids: Vec::new(),
        instructions: None,
    }
}

fn choice(prompt_ids: &[&str]) -> StepSpec {
    StepSpec {
        step_type: "choice".to_string(),
        prompt_id: None,
        choice_prompt_ids: prompt_ids.iter().map(|id| (*id).to_string()).collect(),
        instructions: None,
    }
}

fn assert_invalid<T>(result: Result<T, AppError>) {
    assert!(matches!(result, Err(AppError::Invalid(_))));
}

fn assert_conflict<T>(result: Result<T, AppError>) {
    assert_eq!(
        result.err(),
        Some(AppError::Conflict(
            "End the active session to edit this playbook".to_string()
        ))
    );
}

#[test]
fn patch_fields_distinguish_omitted_null_and_value() {
    let omitted: UpdatePlaybookRequest = serde_json::from_str("{}").unwrap();
    let cleared: UpdatePlaybookRequest = serde_json::from_str(r#"{"description":null}"#).unwrap();
    let set: UpdatePlaybookRequest = serde_json::from_str(r#"{"description":"updated"}"#).unwrap();

    assert_eq!(omitted.description, PatchField::Keep);
    assert_eq!(cleared.description, PatchField::Clear);
    assert_eq!(set.description, PatchField::Set("updated".to_string()));
}

#[test]
fn prompt_and_playbook_descriptions_can_be_cleared() {
    let mut conn = setup_db();
    let prompt_id = create_prompt(&mut conn, "Prompt");
    prompt_service::update_prompt(
        &mut conn,
        &prompt_id,
        UpdatePromptRequest {
            title: None,
            description: PatchField::Set("description".to_string()),
            is_favorite: None,
            is_pinned: None,
            primary_variant_id: None,
        },
    )
    .unwrap();
    prompt_service::update_prompt(
        &mut conn,
        &prompt_id,
        UpdatePromptRequest {
            title: None,
            description: PatchField::Clear,
            is_favorite: None,
            is_pinned: None,
            primary_variant_id: None,
        },
    )
    .unwrap();
    assert_eq!(
        prompt_service::get_prompt_by_id(&conn, &prompt_id)
            .unwrap()
            .prompt
            .description,
        None
    );

    let playbook =
        playbook_service::create_playbook(&mut conn, "Playbook", Some("description")).unwrap();
    let updated = playbook_service::update_playbook(
        &mut conn,
        &playbook.id,
        UpdatePlaybookRequest {
            title: None,
            description: PatchField::Clear,
        },
    )
    .unwrap();
    assert_eq!(updated.description, None);
}

#[test]
fn add_and_update_share_step_validation() {
    let mut conn = setup_db();
    let first = create_prompt(&mut conn, "First");
    let second = create_prompt(&mut conn, "Second");
    let deleted = create_prompt(&mut conn, "Deleted");
    prompt_service::delete_prompt(&mut conn, &deleted).unwrap();
    let playbook = playbook_service::create_playbook(&mut conn, "Playbook", None).unwrap();
    let valid = playbook_service::add_step(&mut conn, &playbook.id, single(&first)).unwrap();

    let invalid_specs = [
        StepSpec {
            step_type: "other".to_string(),
            prompt_id: Some(first.clone()),
            choice_prompt_ids: Vec::new(),
            instructions: None,
        },
        StepSpec {
            step_type: "single".to_string(),
            prompt_id: None,
            choice_prompt_ids: Vec::new(),
            instructions: None,
        },
        StepSpec {
            step_type: "single".to_string(),
            prompt_id: Some(first.clone()),
            choice_prompt_ids: vec![second.clone()],
            instructions: None,
        },
        single("missing"),
        single(&deleted),
        choice(&[&first]),
        choice(&[&first, &first]),
        choice(&[&first, "missing"]),
        StepSpec {
            step_type: "choice".to_string(),
            prompt_id: Some(first.clone()),
            choice_prompt_ids: vec![first.clone(), second.clone()],
            instructions: None,
        },
    ];

    for spec in invalid_specs {
        assert_invalid(playbook_service::add_step(
            &mut conn,
            &playbook.id,
            spec.clone(),
        ));
        assert_invalid(playbook_service::update_step(
            &mut conn,
            &playbook.id,
            &valid.step.id,
            spec,
        ));
    }

    let switched = playbook_service::update_step(
        &mut conn,
        &playbook.id,
        &valid.step.id,
        choice(&[&first, &second]),
    )
    .unwrap();
    assert_eq!(switched.step.prompt_id, None);
    assert_eq!(
        switched.step.choice_prompt_ids,
        vec![first.clone(), second.clone()]
    );

    let switched_back =
        playbook_service::update_step(&mut conn, &playbook.id, &valid.step.id, single(&second))
            .unwrap();
    assert_eq!(
        switched_back.step.prompt_id.as_deref(),
        Some(second.as_str())
    );
    assert!(switched_back.step.choice_prompt_ids.is_empty());
}

#[test]
fn step_parent_ownership_is_enforced_before_active_session_policy() {
    let mut conn = setup_db();
    let prompt_id = create_prompt(&mut conn, "Prompt");
    let active = playbook_service::create_playbook(&mut conn, "Active", None).unwrap();
    let other = playbook_service::create_playbook(&mut conn, "Other", None).unwrap();
    let step = playbook_service::add_step(&mut conn, &active.id, single(&prompt_id)).unwrap();
    playbook_service::start_session(&mut conn, &active.id).unwrap();

    assert_eq!(
        playbook_service::update_step(&mut conn, &other.id, &step.step.id, single(&prompt_id),)
            .unwrap_err(),
        AppError::NotFound
    );
    assert_eq!(
        playbook_service::remove_step(&mut conn, &other.id, &step.step.id).unwrap_err(),
        AppError::NotFound
    );
    assert_conflict(playbook_service::update_step(
        &mut conn,
        &active.id,
        &step.step.id,
        single(&prompt_id),
    ));
}

#[test]
fn structural_edits_conflict_only_for_the_active_playbook_and_resume_after_end() {
    let mut conn = setup_db();
    let first = create_prompt(&mut conn, "First");
    let second = create_prompt(&mut conn, "Second");
    let active = playbook_service::create_playbook(&mut conn, "Active", None).unwrap();
    let other = playbook_service::create_playbook(&mut conn, "Other", None).unwrap();
    let active_step = playbook_service::add_step(&mut conn, &active.id, single(&first)).unwrap();
    let other_step = playbook_service::add_step(&mut conn, &other.id, single(&first)).unwrap();
    playbook_service::start_session(&mut conn, &active.id).unwrap();

    assert_conflict(playbook_service::add_step(
        &mut conn,
        &active.id,
        single(&second),
    ));
    assert_conflict(playbook_service::update_step(
        &mut conn,
        &active.id,
        &active_step.step.id,
        single(&second),
    ));
    assert_conflict(playbook_service::remove_step(
        &mut conn,
        &active.id,
        &active_step.step.id,
    ));
    assert_conflict(playbook_service::reorder_steps(
        &mut conn,
        &active.id,
        std::slice::from_ref(&active_step.step.id),
    ));
    assert_conflict(playbook_service::delete_playbook(&mut conn, &active.id));

    let other_added = playbook_service::add_step(&mut conn, &other.id, single(&second)).unwrap();
    playbook_service::update_step(&mut conn, &other.id, &other_step.step.id, single(&second))
        .unwrap();
    playbook_service::reorder_steps(
        &mut conn,
        &other.id,
        &[other_added.step.id.clone(), other_step.step.id.clone()],
    )
    .unwrap();
    playbook_service::remove_step(&mut conn, &other.id, &other_added.step.id).unwrap();
    playbook_service::delete_playbook(&mut conn, &other.id).unwrap();

    playbook_service::end_session(&mut conn).unwrap();
    let added = playbook_service::add_step(&mut conn, &active.id, single(&second)).unwrap();
    playbook_service::update_step(&mut conn, &active.id, &active_step.step.id, single(&second))
        .unwrap();
    playbook_service::reorder_steps(
        &mut conn,
        &active.id,
        &[added.step.id.clone(), active_step.step.id.clone()],
    )
    .unwrap();
    playbook_service::remove_step(&mut conn, &active.id, &added.step.id).unwrap();
    playbook_service::delete_playbook(&mut conn, &active.id).unwrap();
}

#[test]
fn remove_after_reorder_compacts_positions_without_collisions() {
    let mut conn = setup_db();
    let prompt_id = create_prompt(&mut conn, "Prompt");
    let playbook = playbook_service::create_playbook(&mut conn, "Playbook", None).unwrap();
    let steps = (0..3)
        .map(|_| playbook_service::add_step(&mut conn, &playbook.id, single(&prompt_id)).unwrap())
        .collect::<Vec<_>>();

    playbook_service::reorder_steps(
        &mut conn,
        &playbook.id,
        &[
            steps[2].step.id.clone(),
            steps[0].step.id.clone(),
            steps[1].step.id.clone(),
        ],
    )
    .unwrap();
    playbook_service::remove_step(&mut conn, &playbook.id, &steps[0].step.id).unwrap();

    let current = playbook_service::get_playbook(&conn, &playbook.id).unwrap();
    assert_eq!(
        current
            .steps
            .iter()
            .map(|step| (&step.step.id, step.step.position))
            .collect::<Vec<_>>(),
        vec![(&steps[2].step.id, 0), (&steps[1].step.id, 1)]
    );
}

#[test]
fn deleted_prompt_hydration_preserves_raw_choice_ids_and_filters_resolved_prompts() {
    let mut conn = setup_db();
    let first = create_prompt(&mut conn, "First");
    let second = create_prompt(&mut conn, "Second");
    let third = create_prompt(&mut conn, "Third");
    let playbook = playbook_service::create_playbook(&mut conn, "Playbook", None).unwrap();
    playbook_service::add_step(&mut conn, &playbook.id, single(&first)).unwrap();
    playbook_service::add_step(&mut conn, &playbook.id, choice(&[&first, &second, &third]))
        .unwrap();

    prompt_service::delete_prompt(&mut conn, &first).unwrap();
    let hydrated = playbook_service::get_playbook(&conn, &playbook.id).unwrap();
    assert!(hydrated.steps[0].prompt.is_none());
    assert_eq!(
        hydrated.steps[1].step.choice_prompt_ids,
        vec![first.clone(), second.clone(), third.clone()]
    );
    assert_eq!(
        hydrated.steps[1]
            .choice_prompts
            .iter()
            .map(|prompt| prompt.prompt.id.as_str())
            .collect::<Vec<_>>(),
        vec![second.as_str(), third.as_str()]
    );
}

#[test]
fn prompt_usage_counts_single_and_exact_choice_membership() {
    let mut conn = setup_db();
    let target = create_prompt(&mut conn, "Target");
    let second = create_prompt(&mut conn, "Second");
    let third = create_prompt(&mut conn, "Third");
    let first_playbook = playbook_service::create_playbook(&mut conn, "Alpha", None).unwrap();
    let second_playbook = playbook_service::create_playbook(&mut conn, "Beta", None).unwrap();

    playbook_service::add_step(&mut conn, &first_playbook.id, single(&target)).unwrap();
    playbook_service::add_step(&mut conn, &first_playbook.id, choice(&[&target, &second])).unwrap();
    playbook_service::add_step(&mut conn, &second_playbook.id, choice(&[&target, &third])).unwrap();

    let usage = prompt_service::get_prompt_usage(&conn, &target).unwrap();
    assert_eq!(usage.playbook_count, 2);
    assert_eq!(usage.step_count, 3);
    assert_eq!(usage.playbook_titles, vec!["Alpha", "Beta"]);
}
