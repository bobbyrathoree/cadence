use cadence_lib::db::schema;
use cadence_lib::models::patch::PatchField;
use cadence_lib::models::prompt::{CreatePromptRequest, UpdatePromptRequest};
use cadence_lib::services::{prompt_service, search_service, tag_service};

fn setup_db() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    schema::create_tables(&conn).unwrap();
    conn
}

fn create_prompt(
    conn: &mut rusqlite::Connection,
    title: &str,
    description: Option<&str>,
    content: &str,
    tags: &[&str],
) -> cadence_lib::models::prompt::PromptWithVariants {
    prompt_service::create_prompt(
        conn,
        CreatePromptRequest {
            title: title.to_string(),
            description: description.map(str::to_string),
            content: content.to_string(),
            variant_label: None,
            tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
            is_favorite: false,
        },
    )
    .unwrap()
}

fn result_ids(conn: &rusqlite::Connection, query: &str) -> Vec<String> {
    search_service::search_prompts(conn, query, 50)
        .unwrap()
        .into_iter()
        .map(|item| item.id)
        .collect()
}

#[test]
fn title_change_evicts_old_terms() {
    let mut conn = setup_db();
    let prompt = create_prompt(&mut conn, "Amberquartz title", None, "neutral content", &[]);

    prompt_service::update_prompt(
        &mut conn,
        &prompt.prompt.id,
        UpdatePromptRequest {
            title: Some("Verdant title".to_string()),
            description: PatchField::Keep,
            is_favorite: None,
            is_pinned: None,
            primary_variant_id: None,
        },
    )
    .unwrap();

    assert!(result_ids(&conn, "amberquartz").is_empty());
    assert_eq!(result_ids(&conn, "verdant"), vec![prompt.prompt.id]);
}

#[test]
fn description_change_evicts_old_terms() {
    let mut conn = setup_db();
    let prompt = create_prompt(
        &mut conn,
        "Description change",
        Some("Cobaltmist description"),
        "neutral content",
        &[],
    );

    prompt_service::update_prompt(
        &mut conn,
        &prompt.prompt.id,
        UpdatePromptRequest {
            title: None,
            description: PatchField::Set("Silver description".to_string()),
            is_favorite: None,
            is_pinned: None,
            primary_variant_id: None,
        },
    )
    .unwrap();

    assert!(result_ids(&conn, "cobaltmist").is_empty());
    assert_eq!(result_ids(&conn, "silver"), vec![prompt.prompt.id]);
}

#[test]
fn primary_variant_content_change_evicts_removed_terms() {
    let mut conn = setup_db();
    let prompt = create_prompt(
        &mut conn,
        "Content edit",
        None,
        "oldtoken remains searchable",
        &[],
    );
    let primary_id = prompt.prompt.primary_variant_id.unwrap();

    prompt_service::update_variant(
        &mut conn,
        &primary_id,
        "newtoken replaces the original",
        None,
    )
    .unwrap();

    assert!(result_ids(&conn, "oldtoken").is_empty());
    assert_eq!(result_ids(&conn, "newtoken"), vec![prompt.prompt.id]);
}

#[test]
fn primary_variant_switch_replaces_indexed_content() {
    let mut conn = setup_db();
    let prompt = create_prompt(&mut conn, "Primary switch", None, "firstprimarytoken", &[]);
    let replacement = prompt_service::add_variant(
        &mut conn,
        &prompt.prompt.id,
        "Replacement",
        "secondprimarytoken",
    )
    .unwrap();

    assert_eq!(
        result_ids(&conn, "firstprimarytoken"),
        vec![prompt.prompt.id.clone()]
    );
    assert!(result_ids(&conn, "secondprimarytoken").is_empty());

    prompt_service::update_prompt(
        &mut conn,
        &prompt.prompt.id,
        UpdatePromptRequest {
            title: None,
            description: PatchField::Keep,
            is_favorite: None,
            is_pinned: None,
            primary_variant_id: Some(replacement.id),
        },
    )
    .unwrap();

    assert!(result_ids(&conn, "firstprimarytoken").is_empty());
    assert_eq!(
        result_ids(&conn, "secondprimarytoken"),
        vec![prompt.prompt.id]
    );
}

#[test]
fn tag_add_and_remove_refresh_indexed_terms() {
    let mut conn = setup_db();
    let prompt = create_prompt(&mut conn, "Tag mutation", None, "neutral content", &[]);

    tag_service::add_tags_to_prompt(&mut conn, &prompt.prompt.id, &["model:claude".to_string()])
        .unwrap();
    assert_eq!(result_ids(&conn, "claude"), vec![prompt.prompt.id.clone()]);

    let tag = tag_service::get_tags_for_prompt(&conn, &prompt.prompt.id)
        .unwrap()
        .into_iter()
        .find(|tag| tag.name == "model:claude")
        .unwrap();
    tag_service::remove_tag_from_prompt(&mut conn, &prompt.prompt.id, &tag.id).unwrap();

    assert!(result_ids(&conn, "claude").is_empty());
}

#[test]
fn deleting_primary_variant_promotes_and_reindexes_replacement() {
    let mut conn = setup_db();
    let prompt = create_prompt(&mut conn, "Variant promotion", None, "departingtoken", &[]);
    let primary_id = prompt.prompt.primary_variant_id.unwrap();
    prompt_service::add_variant(&mut conn, &prompt.prompt.id, "Replacement", "promotedtoken")
        .unwrap();

    prompt_service::delete_variant(&mut conn, &primary_id).unwrap();

    assert!(result_ids(&conn, "departingtoken").is_empty());
    assert_eq!(result_ids(&conn, "promotedtoken"), vec![prompt.prompt.id]);
}

#[test]
fn soft_delete_evicts_search_row_and_mapping() {
    let mut conn = setup_db();
    let prompt = create_prompt(&mut conn, "Soft delete", None, "confidentialtoken", &[]);

    prompt_service::delete_prompt(&mut conn, &prompt.prompt.id).unwrap();

    assert!(result_ids(&conn, "confidentialtoken").is_empty());
    let mapping_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM fts_mapping WHERE prompt_id = ?1",
            [&prompt.prompt.id],
            |row| row.get(0),
        )
        .unwrap();
    let fts_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM prompts_fts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mapping_count, 0);
    assert_eq!(fts_count, 0);
}
