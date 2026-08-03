use std::collections::BTreeSet;

use cadence_core::db::schema;
use cadence_core::models::collection::CreateCollectionRequest;
use cadence_core::models::prompt::{CreatePromptRequest, PromptListItem};
use cadence_core::services::{collection_service, prompt_service, search_service};

fn setup_db() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    schema::create_tables(&conn).unwrap();
    conn
}

fn create_prompt(
    conn: &mut rusqlite::Connection,
    title: &str,
    content: &str,
    tags: &[&str],
) -> String {
    prompt_service::create_prompt(
        conn,
        CreatePromptRequest {
            title: title.to_string(),
            description: None,
            content: content.to_string(),
            variant_label: None,
            tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
            is_favorite: false,
        },
    )
    .unwrap()
    .prompt
    .id
}

fn result_ids(conn: &rusqlite::Connection, query: &str) -> BTreeSet<String> {
    search_service::search_prompts(conn, query, 50)
        .unwrap()
        .into_iter()
        .map(|item| item.id)
        .collect()
}

fn expected(ids: &[&String]) -> BTreeSet<String> {
    ids.iter().map(|id| (*id).clone()).collect()
}

#[test]
fn punctuation_queries_use_allowlisted_tokens_implicit_and_and_final_prefix() {
    let mut conn = setup_db();
    let node = create_prompt(&mut conn, "One", "node expressive", &[]);
    create_prompt(&mut conn, "Two", "nodejs expressive", &[]);
    let what = create_prompt(&mut conn, "Three", "whatsoever", &[]);
    let eg = create_prompt(&mut conn, "Four", "e garden", &[]);
    create_prompt(&mut conn, "Five", "example garden", &[]);
    let fifty = create_prompt(&mut conn, "Six", "500 reasons", &[]);
    let and = create_prompt(&mut conn, "Seven", "android platform", &[]);
    let ampersand = create_prompt(&mut conn, "Eight", "a beta", &[]);
    create_prompt(&mut conn, "Nine", "alpha beta", &[]);
    let tagged = create_prompt(&mut conn, "Ten", "neutral content", &["model:claude"]);

    assert_eq!(result_ids(&conn, "node, express"), expected(&[&node]));
    assert_eq!(result_ids(&conn, "what?"), expected(&[&what]));
    assert_eq!(result_ids(&conn, "e.g."), expected(&[&eg]));
    assert_eq!(result_ids(&conn, "50%"), expected(&[&fifty]));
    assert_eq!(result_ids(&conn, "AND"), expected(&[&and]));
    assert_eq!(result_ids(&conn, "a & b"), expected(&[&ampersand]));
    assert_eq!(result_ids(&conn, "model:claude"), expected(&[&tagged]));
    assert!(result_ids(&conn, "!@#$%^&*()").is_empty());
}

#[test]
fn astral_emoji_before_match_keeps_snippet_runs_on_character_boundaries() {
    let mut conn = setup_db();
    let content = "🎸🎸 guitar riff";
    create_prompt(&mut conn, "Music", content, &[]);

    let result = search_service::search_prompts(&conn, "guitar", 50)
        .unwrap()
        .pop()
        .unwrap();
    let rejoined = result
        .snippet_runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<String>();
    let highlights = result
        .snippet_runs
        .iter()
        .filter(|run| run.highlighted)
        .map(|run| run.text.as_str())
        .collect::<Vec<_>>();

    assert_eq!(rejoined, content);
    assert_eq!(highlights, vec!["guitar"]);
    assert_eq!(result.snippet, content);
}

#[test]
fn cjk_match_window_remains_valid_and_highlights_the_complete_token() {
    let mut conn = setup_db();
    let content = format!("{} 吉他 {}", "界".repeat(65), "声".repeat(70));
    create_prompt(&mut conn, "CJK", &content, &[]);

    let result = search_service::search_prompts(&conn, "吉他", 50)
        .unwrap()
        .pop()
        .unwrap();
    let rejoined = result
        .snippet_runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<String>();
    let expected_window = content.chars().collect::<Vec<_>>()[6..128]
        .iter()
        .collect::<String>();
    let highlights = result
        .snippet_runs
        .iter()
        .filter(|run| run.highlighted)
        .map(|run| run.text.as_str())
        .collect::<Vec<_>>();

    assert_eq!(rejoined, expected_window);
    assert_eq!(highlights, vec!["吉他"]);
    assert_eq!(result.snippet, expected_window);
}

#[test]
fn title_only_match_uses_plain_first_120_content_characters() {
    let mut conn = setup_db();
    let content = "内容".repeat(70);
    create_prompt(&mut conn, "Fallbacktitle metadata", &content, &[]);

    let result = search_service::search_prompts(&conn, "fallbacktitle", 50)
        .unwrap()
        .pop()
        .unwrap();
    let expected = content.chars().take(120).collect::<String>();

    assert_eq!(result.snippet, expected);
    assert_eq!(result.snippet_runs.len(), 1);
    assert_eq!(result.snippet_runs[0].text, expected);
    assert!(!result.snippet_runs[0].highlighted);
}

#[test]
fn list_and_collection_items_preserve_legacy_snippets_with_empty_runs() {
    let mut conn = setup_db();
    let content = "x".repeat(130);
    let prompt_id = create_prompt(&mut conn, "Compatibility", &content, &[]);
    let collection = collection_service::create_collection(
        &mut conn,
        CreateCollectionRequest {
            name: "Manual".to_string(),
            description: None,
            icon: None,
            color: None,
            is_smart: false,
            filter_query: None,
        },
    )
    .unwrap();
    collection_service::add_prompt_to_collection(&mut conn, &collection.id, &prompt_id).unwrap();

    let list_item = prompt_service::list_prompts(&conn, 100, 0)
        .unwrap()
        .pop()
        .unwrap();
    let collection_item = collection_service::get_collection_prompts(&conn, &collection.id, 100, 0)
        .unwrap()
        .pop()
        .unwrap();

    assert_eq!(list_item.snippet, "x".repeat(100));
    assert!(list_item.snippet_runs.is_empty());
    assert_eq!(collection_item.snippet, "x".repeat(100));
    assert!(collection_item.snippet_runs.is_empty());
}

#[test]
fn legacy_prompt_list_payload_defaults_snippet_runs_to_empty() {
    let item: PromptListItem = serde_json::from_value(serde_json::json!({
        "id": "prompt-1",
        "title": "Legacy",
        "description": null,
        "snippet": "legacy snippet",
        "is_favorite": false,
        "variant_count": 1,
        "copy_count": 0,
        "last_copied_at": null,
        "tags": []
    }))
    .unwrap();

    assert!(item.snippet_runs.is_empty());
}
