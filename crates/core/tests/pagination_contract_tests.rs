use cadence_core::db::{schema, Db, Health};
use cadence_core::models::collection::CreateCollectionRequest;
use cadence_core::models::prompt::CreatePromptRequest;
use cadence_core::services::{collection_service, pagination, prompt_service, search_service};
use rusqlite::{params, Connection};

fn setup_db() -> Db {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    schema::create_tables(&conn).unwrap();
    Db {
        conn,
        health: Health::exit_process(1),
    }
}

fn insert_prompt(
    conn: &Connection,
    id: &str,
    updated_at: &str,
    favorite: bool,
    last_copied_at: Option<&str>,
) {
    conn.execute(
        "INSERT INTO prompts
            (id, title, is_favorite, copy_count, last_copied_at, created_at, updated_at)
         VALUES (?1, ?1, ?2, 0, ?3, ?4, ?4)",
        params![id, favorite as i64, last_copied_at, updated_at],
    )
    .unwrap();
}

fn ids(items: &[cadence_core::models::prompt::PromptListItem]) -> Vec<String> {
    items.iter().map(|item| item.id.clone()).collect()
}

#[test]
fn prompt_views_use_exact_pinned_ordering_across_deep_offsets() {
    let conn = setup_db();
    for index in 0..1_200 {
        let id = format!("prompt-{index:04}");
        let copied = match index {
            7 => Some("2026-08-02T01:00:00Z"),
            8 | 9 => Some("2026-08-02T02:00:00Z"),
            _ => None,
        };
        insert_prompt(
            &conn,
            &id,
            &format!("2026-08-02T00:{index:04}:00Z"),
            index == 49,
            copied,
        );
    }

    for offset in [0, 100, 1_100] {
        let actual =
            prompt_service::list_prompts_page(&conn, Some("all"), Some(100), Some(offset)).unwrap();
        let start = 1_199 - offset;
        let end = (start - 99).max(0);
        let expected = (end..=start)
            .rev()
            .map(|index| format!("prompt-{index:04}"))
            .collect::<Vec<_>>();
        assert_eq!(ids(&actual), expected, "offset {offset}");
    }

    let favorites =
        prompt_service::list_prompts_page(&conn, Some("favorites"), Some(100), Some(0)).unwrap();
    assert_eq!(ids(&favorites), vec!["prompt-0049"]);

    let recents = prompt_service::list_prompts_page(&conn, Some("recent"), None, None).unwrap();
    assert_eq!(
        ids(&recents),
        vec!["prompt-0009", "prompt-0008", "prompt-0007"]
    );
    assert!(recents.iter().all(|item| item.last_copied_at.is_some()));

    let counts = prompt_service::get_prompt_counts(&conn).unwrap();
    assert_eq!(counts.all, 1_200);
    assert_eq!(counts.favorites, 1);
    assert_eq!(counts.recents, 3);
}

#[test]
fn manual_collection_pages_preserve_exact_curated_positions() {
    let mut conn = setup_db();
    let collection = collection_service::create_collection(
        &mut conn,
        CreateCollectionRequest {
            name: "Curated".to_string(),
            description: None,
            icon: None,
            color: None,
            is_smart: false,
            filter_query: None,
        },
    )
    .unwrap();

    for position in 0..250 {
        let id = format!("member-{:03}", 249 - position);
        insert_prompt(&conn, &id, "2026-08-02T00:00:00Z", false, None);
        conn.execute(
            "INSERT INTO collection_prompts (collection_id, prompt_id, position)
             VALUES (?1, ?2, ?3)",
            params![collection.id, id, position],
        )
        .unwrap();
    }

    for (offset, length) in [(0, 100), (100, 100), (200, 50)] {
        let page = collection_service::get_collection_prompts_page(
            &conn,
            &collection.id,
            Some(100),
            Some(offset),
        )
        .unwrap();
        let expected = (offset..offset + length)
            .map(|position| format!("member-{:03}", 249 - position))
            .collect::<Vec<_>>();
        assert_eq!(ids(&page), expected, "offset {offset}");
    }
}

#[test]
fn smart_collections_use_updated_at_then_id_descending() {
    let mut conn = setup_db();
    let collection = collection_service::create_collection(
        &mut conn,
        CreateCollectionRequest {
            name: "Favorites".to_string(),
            description: None,
            icon: None,
            color: None,
            is_smart: true,
            filter_query: Some(
                r#"{"conditions":[{"field":"is_favorite","op":"eq","value":true}],"match":"all"}"#
                    .to_string(),
            ),
        },
    )
    .unwrap();
    insert_prompt(&conn, "older", "2026-08-01", true, None);
    insert_prompt(&conn, "same-a", "2026-08-02", true, None);
    insert_prompt(&conn, "same-b", "2026-08-02", true, None);

    let page =
        collection_service::get_collection_prompts_page(&conn, &collection.id, None, None).unwrap();
    assert_eq!(ids(&page), vec!["same-b", "same-a", "older"]);
}

#[test]
fn omitted_pagination_preserves_legacy_array_shape_defaults_and_fields() {
    let conn = setup_db();
    for index in 0..120 {
        insert_prompt(
            &conn,
            &format!("prompt-{index:03}"),
            &format!("{index:03}"),
            index == 1,
            None,
        );
    }

    let legacy = prompt_service::list_prompts(&conn, 100, 0).unwrap();
    let omitted = prompt_service::list_prompts_page(&conn, None, None, None).unwrap();
    assert_eq!(ids(&legacy), ids(&omitted));
    assert_eq!(omitted.len(), 100);

    let payload = serde_json::to_value(&omitted).unwrap();
    let rows = payload.as_array().expect("bare array response");
    let first = rows[0].as_object().unwrap();
    for field in [
        "id",
        "title",
        "description",
        "snippet",
        "snippet_runs",
        "is_favorite",
        "variant_count",
        "copy_count",
        "last_copied_at",
        "tags",
    ] {
        assert!(
            first.contains_key(field),
            "missing legacy/additive field {field}"
        );
    }
}

#[test]
fn pagination_validation_and_search_cap_are_shared_service_contracts() {
    assert!(pagination::sanitize_pagination(Some(0), None).is_err());
    assert!(pagination::sanitize_pagination(None, Some(-1)).is_err());
    assert_eq!(
        pagination::sanitize_pagination(Some(5_000), Some(2)).unwrap(),
        (500, 2)
    );
    assert!(prompt_service::list_prompts_page(&setup_db(), Some("unknown"), None, None).is_err());

    let mut conn = setup_db();
    for index in 0..110 {
        prompt_service::create_prompt(
            &mut conn,
            CreatePromptRequest {
                title: format!("Shared token {index:03}"),
                description: None,
                content: "sharedtoken".to_string(),
                variant_label: None,
                tags: Vec::new(),
                is_favorite: false,
            },
        )
        .unwrap();
    }
    let results = search_service::search_prompts_page(&conn, "sharedtoken", Some(1_000)).unwrap();
    assert_eq!(results.len(), 100);
}
