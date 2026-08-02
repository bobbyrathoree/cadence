use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use cadence_lib::db::{self, schema};
use cadence_lib::error::AppError;
use cadence_lib::models::collection::CreateCollectionRequest;
use cadence_lib::models::prompt::{CreatePromptRequest, UpdatePromptRequest};
use cadence_lib::seed;
use cadence_lib::services::{
    collection_service, import_export, playbook_service, prompt_service, settings_service,
};
use rusqlite::{params, Connection, TransactionBehavior};

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    schema::create_tables(&conn).unwrap();
    conn
}

fn prompt_request(title: &str) -> CreatePromptRequest {
    CreatePromptRequest {
        title: title.to_string(),
        description: None,
        content: format!("{title} content"),
        variant_label: None,
        tags: vec!["test".to_string()],
        is_favorite: false,
    }
}

fn create_prompt(conn: &mut Connection, title: &str) -> String {
    prompt_service::create_prompt(conn, prompt_request(title))
        .unwrap()
        .prompt
        .id
}

fn create_collection(conn: &mut Connection) -> String {
    collection_service::create_collection(
        conn,
        CreateCollectionRequest {
            name: "Manual".to_string(),
            description: None,
            icon: None,
            color: None,
            is_smart: false,
            filter_query: None,
        },
    )
    .unwrap()
    .id
}

fn assert_not_found<T>(result: Result<T, AppError>) {
    assert_eq!(result.err(), Some(AppError::NotFound));
}

#[test]
fn create_prompt_failure_rolls_back_every_write() {
    let mut conn = setup_db();
    conn.execute_batch(
        "CREATE TRIGGER fail_variant_insert
         BEFORE INSERT ON variants
         BEGIN
           SELECT RAISE(ABORT, 'injected create failure');
         END;",
    )
    .unwrap();

    assert!(prompt_service::create_prompt(&mut conn, prompt_request("Atomic")).is_err());
    for table in ["prompts", "variants", "tags", "prompt_tags", "fts_mapping"] {
        let count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "{table} should remain empty");
    }
}

#[test]
fn delete_variant_failure_restores_primary_and_variant() {
    let mut conn = setup_db();
    let created = prompt_service::create_prompt(&mut conn, prompt_request("Variants")).unwrap();
    let original_primary = created.prompt.primary_variant_id.unwrap();
    let replacement =
        prompt_service::add_variant(&mut conn, &created.prompt.id, "Second", "second").unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_variant_delete
         BEFORE UPDATE OF deleted_at ON variants
         WHEN NEW.deleted_at IS NOT NULL
         BEGIN
           SELECT RAISE(ABORT, 'injected delete failure');
         END;",
    )
    .unwrap();

    assert!(prompt_service::delete_variant(&mut conn, &original_primary).is_err());
    let current = prompt_service::get_prompt_by_id(&conn, &created.prompt.id).unwrap();
    assert_eq!(
        current.prompt.primary_variant_id.as_deref(),
        Some(original_primary.as_str())
    );
    assert!(current
        .variants
        .iter()
        .any(|variant| variant.id == original_primary));
    assert!(current
        .variants
        .iter()
        .any(|variant| variant.id == replacement.id));
}

#[test]
fn remove_step_failure_restores_step_and_positions() {
    let mut conn = setup_db();
    let playbook = playbook_service::create_playbook(&mut conn, "Atomic", None).unwrap();
    let steps = (0..3)
        .map(|index| {
            playbook_service::add_step(
                &mut conn,
                &playbook.id,
                None,
                "instruction",
                Some(&format!("step {index}")),
                None,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    conn.execute_batch(
        "CREATE TRIGGER fail_playbook_touch
         BEFORE UPDATE OF updated_at ON playbooks
         BEGIN
           SELECT RAISE(ABORT, 'injected remove failure');
         END;",
    )
    .unwrap();

    assert!(playbook_service::remove_step(&mut conn, &steps[1].id).is_err());
    let current = playbook_service::get_playbook(&conn, &playbook.id).unwrap();
    assert_eq!(current.steps.len(), 3);
    assert_eq!(
        current
            .steps
            .iter()
            .map(|step| step.step.position)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn import_failure_rolls_back_the_whole_chunk() {
    let mut conn = setup_db();
    conn.execute_batch(
        "CREATE TRIGGER fail_second_import
         BEFORE INSERT ON prompts
         WHEN NEW.title = 'Second'
         BEGIN
           SELECT RAISE(ABORT, 'injected import failure');
         END;",
    )
    .unwrap();
    let json = r#"{
        "prompts": [
            {"title": "First", "content": "one"},
            {"title": "Second", "content": "two"}
        ]
    }"#;

    assert!(import_export::import_json(&mut conn, json).is_err());
    let prompt_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM prompts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(prompt_count, 0);
}

#[test]
fn targeted_mutations_return_not_found_for_unknown_ids() {
    let mut conn = setup_db();
    assert_not_found(prompt_service::update_prompt(
        &mut conn,
        "missing",
        UpdatePromptRequest {
            title: Some("new".to_string()),
            description: None,
            is_favorite: None,
            is_pinned: None,
            primary_variant_id: None,
        },
    ));
    assert_not_found(prompt_service::delete_prompt(&mut conn, "missing"));
    assert_not_found(prompt_service::toggle_favorite(&mut conn, "missing"));
    assert_not_found(prompt_service::toggle_pinned(&mut conn, "missing"));
    assert_not_found(prompt_service::record_copy(&mut conn, "missing", None));
    assert_not_found(prompt_service::add_variant(
        &mut conn, "missing", "label", "content",
    ));
    assert_not_found(prompt_service::update_variant(
        &mut conn, "missing", "content", None,
    ));
    assert_not_found(prompt_service::delete_variant(&mut conn, "missing"));

    assert_not_found(collection_service::add_prompt_to_collection(
        &mut conn, "missing", "missing",
    ));
    assert_not_found(playbook_service::update_playbook(
        &mut conn,
        "missing",
        Some("title"),
        None,
    ));
    assert_not_found(playbook_service::delete_playbook(&mut conn, "missing"));
    assert_not_found(playbook_service::remove_step(&mut conn, "missing"));
    assert_not_found(playbook_service::start_session(&mut conn, "missing"));
    assert_not_found(playbook_service::advance_step(&mut conn));
}

#[test]
fn sqlite_constraints_map_to_conflict() {
    let mut conn = setup_db();
    conn.execute_batch(
        "CREATE TRIGGER reject_prompt
         BEFORE INSERT ON prompts
         BEGIN
           SELECT RAISE(ABORT, 'schema detail must not escape');
         END;",
    )
    .unwrap();
    assert!(matches!(
        prompt_service::create_prompt(&mut conn, prompt_request("Conflict")),
        Err(AppError::Conflict(_))
    ));
}

#[test]
fn reset_shortcuts_is_idempotent_when_row_is_absent() {
    let mut conn = setup_db();
    assert!(settings_service::reset_shortcuts(&mut conn).is_ok());
}

#[test]
fn duplicate_collection_add_is_an_idempotent_no_op() {
    let mut conn = setup_db();
    let prompt_id = create_prompt(&mut conn, "Member");
    let collection_id = create_collection(&mut conn);
    collection_service::add_prompt_to_collection(&mut conn, &collection_id, &prompt_id).unwrap();
    collection_service::add_prompt_to_collection(&mut conn, &collection_id, &prompt_id).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM collection_prompts
             WHERE collection_id = ?1 AND prompt_id = ?2",
            params![collection_id, prompt_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn absent_collection_membership_remove_is_an_idempotent_no_op() {
    let mut conn = setup_db();
    let prompt_id = create_prompt(&mut conn, "Member");
    let collection_id = create_collection(&mut conn);
    assert!(collection_service::remove_prompt_from_collection(
        &mut conn,
        &collection_id,
        &prompt_id,
    )
    .is_ok());
}

#[test]
fn setting_api_enabled_to_current_value_is_an_idempotent_no_op() {
    let mut conn = setup_db();
    settings_service::set_api_enabled(&mut conn, false).unwrap();
    assert!(
        settings_service::get_setting(&conn, settings_service::API_ENABLED_KEY)
            .unwrap()
            .is_none()
    );
    settings_service::set_api_enabled(&mut conn, true).unwrap();
    settings_service::set_api_enabled(&mut conn, true).unwrap();
    assert!(settings_service::get_api_enabled(&conn).unwrap());
}

#[test]
fn session_read_does_not_write() {
    let conn = setup_db();
    let changes_before: i64 = conn
        .query_row("SELECT total_changes()", [], |row| row.get(0))
        .unwrap();
    let session = playbook_service::get_session(&conn).unwrap();
    assert_eq!(session.id, 1);
    let changes_after: i64 = conn
        .query_row("SELECT total_changes()", [], |row| row.get(0))
        .unwrap();
    assert_eq!(changes_after, changes_before);
}

#[test]
fn seed_marker_prevents_reseeding_after_every_prompt_is_deleted() {
    let mut conn = setup_db();
    seed::seed_if_empty(&mut conn).unwrap();
    let prompt_ids = prompt_service::list_prompts(&conn, 100, 0)
        .unwrap()
        .into_iter()
        .map(|prompt| prompt.id)
        .collect::<Vec<_>>();
    assert!(!prompt_ids.is_empty());
    for prompt_id in prompt_ids {
        prompt_service::delete_prompt(&mut conn, &prompt_id).unwrap();
    }

    seed::seed_if_empty(&mut conn).unwrap();
    assert!(prompt_service::list_prompts(&conn, 100, 0)
        .unwrap()
        .is_empty());
    assert!(settings_service::get_setting(&conn, "seeded_at")
        .unwrap()
        .is_some());
}

#[test]
fn two_wal_connections_wait_for_short_writer_then_succeed() {
    let path = unique_db_path("wal-contention");
    let mut first = db::connect(&path).unwrap();
    let mut second = db::connect(&path).unwrap();

    for conn in [&first, &second] {
        assert_eq!(
            conn.pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))
                .unwrap()
                .to_lowercase(),
            "wal"
        );
        assert_eq!(
            conn.pragma_query_value(None, "busy_timeout", |row| row.get::<_, i64>(0))
                .unwrap(),
            5000
        );
        assert_eq!(
            conn.pragma_query_value(None, "synchronous", |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    let first_write = first
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    first_write
        .execute(
            "INSERT INTO settings (key, value) VALUES ('first', 'held')",
            [],
        )
        .unwrap();

    let (started_tx, started_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let started = Instant::now();
        let result = settings_service::set_setting(&mut second, "second", "completed");
        result_tx.send((result, started.elapsed())).unwrap();
    });

    started_rx.recv().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    first_write.commit().unwrap();

    let (result, elapsed) = result_rx.recv().unwrap();
    worker.join().unwrap();
    assert!(result.is_ok());
    assert!(elapsed >= Duration::from_millis(75));
    assert_eq!(
        settings_service::get_setting(&first, "second")
            .unwrap()
            .as_deref(),
        Some("completed")
    );

    drop(first);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
}

fn unique_db_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "cadence-{label}-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}
