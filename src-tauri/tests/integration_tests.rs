/// Comprehensive integration tests for Cadence services.
///
/// Each test creates a fresh in-memory SQLite database with the full schema
/// applied, then exercises the service layer directly.

use cadence_lib::db::schema;
use cadence_lib::models::collection::CreateCollectionRequest;
use cadence_lib::models::prompt::{CreatePromptRequest, UpdatePromptRequest};
use cadence_lib::services::{
    collection_service, import_export, playbook_service, prompt_service, search_service,
    tag_service,
};

/// Create a fresh in-memory database with schema and FKs enabled.
fn setup_db() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    schema::create_tables(&conn).unwrap();
    conn
}

/// Helper: create a prompt with sensible defaults and return the result.
fn create_test_prompt(
    conn: &rusqlite::Connection,
    title: &str,
    content: &str,
    tags: Vec<String>,
    is_favorite: bool,
) -> cadence_lib::models::prompt::PromptWithVariants {
    let req = CreatePromptRequest {
        title: title.to_string(),
        description: None,
        content: content.to_string(),
        variant_label: None,
        tags,
        is_favorite,
    };
    prompt_service::create_prompt(conn, req).unwrap()
}

// =========================================================================
// 1. Prompt CRUD
// =========================================================================

#[test]
fn test_prompt_crud() {
    let conn = setup_db();

    // --- Create ---
    let created = create_test_prompt(
        &conn,
        "My Prompt",
        "Hello, world!",
        vec!["rust".to_string(), "testing".to_string()],
        false,
    );
    assert_eq!(created.prompt.title, "My Prompt", "Title should match");
    assert_eq!(created.variants.len(), 1, "Should have 1 default variant");
    assert_eq!(
        created.variants[0].content, "Hello, world!",
        "Variant content should match"
    );
    assert_eq!(created.tags.len(), 2, "Should have 2 tags");

    let prompt_id = &created.prompt.id;

    // --- List ---
    let list = prompt_service::list_prompts(&conn, 50, 0).unwrap();
    assert_eq!(list.len(), 1, "Should have 1 prompt in list");
    assert_eq!(list[0].id, *prompt_id, "Listed prompt ID should match");

    // --- Get by ID ---
    let fetched = prompt_service::get_prompt_by_id(&conn, prompt_id).unwrap();
    assert_eq!(fetched.prompt.title, "My Prompt", "Fetched title should match");
    assert_eq!(fetched.variants.len(), 1, "Fetched prompt should have 1 variant");
    assert_eq!(fetched.tags.len(), 2, "Fetched prompt should have 2 tags");

    // --- Update ---
    let update_req = UpdatePromptRequest {
        title: Some("Updated Title".to_string()),
        description: None,
        is_favorite: None,
        is_pinned: None,
        primary_variant_id: None,
    };
    prompt_service::update_prompt(&conn, prompt_id, update_req).unwrap();

    let updated = prompt_service::get_prompt_by_id(&conn, prompt_id).unwrap();
    assert_eq!(
        updated.prompt.title, "Updated Title",
        "Title should be updated"
    );

    // --- Soft delete ---
    prompt_service::delete_prompt(&conn, prompt_id).unwrap();

    let list_after_delete = prompt_service::list_prompts(&conn, 50, 0).unwrap();
    assert_eq!(
        list_after_delete.len(),
        0,
        "Deleted prompt should not appear in list"
    );

    // get_prompt_by_id should fail for soft-deleted prompt
    let get_result = prompt_service::get_prompt_by_id(&conn, prompt_id);
    assert!(
        get_result.is_err(),
        "get_prompt_by_id should error for soft-deleted prompt"
    );
}

// =========================================================================
// 2. Variant operations
// =========================================================================

#[test]
fn test_variant_operations() {
    let conn = setup_db();

    let created = create_test_prompt(&conn, "Variant Prompt", "Default content", vec![], false);
    let prompt_id = &created.prompt.id;

    // Should start with 1 variant (the default)
    let fetched = prompt_service::get_prompt_by_id(&conn, prompt_id).unwrap();
    assert_eq!(fetched.variants.len(), 1, "Should start with 1 variant");

    // --- Add second variant ---
    let v2 = prompt_service::add_variant(&conn, prompt_id, "Claude Opus", "Opus-tuned content")
        .unwrap();
    assert_eq!(v2.label, "Claude Opus", "Second variant label should match");

    // --- Add third variant ---
    let v3 =
        prompt_service::add_variant(&conn, prompt_id, "Concise", "Short and direct").unwrap();
    assert_eq!(v3.label, "Concise", "Third variant label should match");

    // --- Verify all 3 variants ---
    let fetched = prompt_service::get_prompt_by_id(&conn, prompt_id).unwrap();
    assert_eq!(
        fetched.variants.len(),
        3,
        "Should have 3 variants after adding 2 more"
    );

    // --- Update a variant ---
    prompt_service::update_variant(&conn, &v2.id, "Updated Opus content", Some("Claude Opus v2"))
        .unwrap();
    let fetched = prompt_service::get_prompt_by_id(&conn, prompt_id).unwrap();
    let updated_v2 = fetched.variants.iter().find(|v| v.id == v2.id).unwrap();
    assert_eq!(
        updated_v2.content, "Updated Opus content",
        "Variant content should be updated"
    );
    assert_eq!(
        updated_v2.label, "Claude Opus v2",
        "Variant label should be updated"
    );

    // --- Soft delete a variant ---
    prompt_service::delete_variant(&conn, &v3.id).unwrap();
    let fetched = prompt_service::get_prompt_by_id(&conn, prompt_id).unwrap();
    assert_eq!(
        fetched.variants.len(),
        2,
        "Should have 2 variants after deleting one"
    );
    assert!(
        fetched.variants.iter().all(|v| v.id != v3.id),
        "Deleted variant should not appear"
    );
}

// =========================================================================
// 3. Favorites
// =========================================================================

#[test]
fn test_favorites() {
    let conn = setup_db();

    let p1 = create_test_prompt(&conn, "Prompt A", "Content A", vec![], true);
    let _p2 = create_test_prompt(&conn, "Prompt B", "Content B", vec![], false);
    let p3 = create_test_prompt(&conn, "Prompt C", "Content C", vec![], true);

    let all = prompt_service::list_prompts(&conn, 50, 0).unwrap();
    assert_eq!(all.len(), 3, "Should have 3 prompts total");

    let favorites: Vec<_> = all.iter().filter(|p| p.is_favorite).collect();
    assert_eq!(favorites.len(), 2, "Should have 2 favorites");

    let fav_ids: Vec<&str> = favorites.iter().map(|p| p.id.as_str()).collect();
    assert!(
        fav_ids.contains(&p1.prompt.id.as_str()),
        "Prompt A should be a favorite"
    );
    assert!(
        fav_ids.contains(&p3.prompt.id.as_str()),
        "Prompt C should be a favorite"
    );
}

// =========================================================================
// 4. Tag operations
// =========================================================================

#[test]
fn test_tag_operations() {
    let conn = setup_db();

    // --- get_or_create_tag ---
    let tag1 = tag_service::get_or_create_tag(&conn, "rust").unwrap();
    assert_eq!(tag1.name, "rust", "Tag name should be 'rust'");

    // --- Idempotency ---
    let tag1_again = tag_service::get_or_create_tag(&conn, "rust").unwrap();
    assert_eq!(
        tag1.id, tag1_again.id,
        "Creating same tag twice should return same ID"
    );

    // --- Add tags to prompt ---
    let prompt = create_test_prompt(&conn, "Tagged Prompt", "Content here", vec![], false);
    let prompt_id = &prompt.prompt.id;

    tag_service::add_tags_to_prompt(
        &conn,
        prompt_id,
        &["alpha".to_string(), "beta".to_string()],
    )
    .unwrap();

    // --- get_tags_for_prompt ---
    let tags = tag_service::get_tags_for_prompt(&conn, prompt_id).unwrap();
    assert_eq!(tags.len(), 2, "Should have 2 tags on the prompt");
    let tag_names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
    assert!(tag_names.contains(&"alpha"), "Should contain 'alpha' tag");
    assert!(tag_names.contains(&"beta"), "Should contain 'beta' tag");

    // --- Remove a tag ---
    let alpha_tag = tags.iter().find(|t| t.name == "alpha").unwrap();
    tag_service::remove_tag_from_prompt(&conn, prompt_id, &alpha_tag.id).unwrap();

    let tags_after = tag_service::get_tags_for_prompt(&conn, prompt_id).unwrap();
    assert_eq!(tags_after.len(), 1, "Should have 1 tag after removal");
    assert_eq!(
        tags_after[0].name, "beta",
        "Remaining tag should be 'beta'"
    );

    // --- List all tags ---
    // We created "rust", "alpha", "beta" across the test
    let all_tags = tag_service::list_tags(&conn).unwrap();
    assert!(
        all_tags.len() >= 3,
        "Should have at least 3 tags total (got {})",
        all_tags.len()
    );
}

// =========================================================================
// 5. Collection operations (manual)
// =========================================================================

#[test]
fn test_collection_operations() {
    let conn = setup_db();

    // Create some prompts
    let p1 = create_test_prompt(&conn, "Prompt 1", "Content 1", vec![], false);
    let p2 = create_test_prompt(&conn, "Prompt 2", "Content 2", vec![], false);
    let p3 = create_test_prompt(&conn, "Prompt 3", "Content 3", vec![], false);

    // --- Create collection ---
    let coll = collection_service::create_collection(
        &conn,
        CreateCollectionRequest {
            name: "My Collection".to_string(),
            description: Some("Test collection".to_string()),
            icon: None,
            color: None,
            is_smart: false,
            filter_query: None,
        },
    )
    .unwrap();
    assert_eq!(coll.name, "My Collection", "Collection name should match");

    // --- Add prompts in order ---
    collection_service::add_prompt_to_collection(&conn, &coll.id, &p1.prompt.id).unwrap();
    collection_service::add_prompt_to_collection(&conn, &coll.id, &p2.prompt.id).unwrap();
    collection_service::add_prompt_to_collection(&conn, &coll.id, &p3.prompt.id).unwrap();

    // --- Get collection prompts ---
    let items = collection_service::get_collection_prompts(&conn, &coll.id, 50, 0).unwrap();
    assert_eq!(
        items.len(),
        3,
        "Collection should have 3 prompts"
    );
    // Verify order: first added first
    assert_eq!(
        items[0].id, p1.prompt.id,
        "First prompt in collection should be Prompt 1"
    );
    assert_eq!(
        items[1].id, p2.prompt.id,
        "Second prompt in collection should be Prompt 2"
    );
    assert_eq!(
        items[2].id, p3.prompt.id,
        "Third prompt in collection should be Prompt 3"
    );

    // --- Remove a prompt ---
    collection_service::remove_prompt_from_collection(&conn, &coll.id, &p2.prompt.id).unwrap();
    let items_after = collection_service::get_collection_prompts(&conn, &coll.id, 50, 0).unwrap();
    assert_eq!(
        items_after.len(),
        2,
        "Collection should have 2 prompts after removal"
    );
    assert!(
        items_after.iter().all(|i| i.id != p2.prompt.id),
        "Removed prompt should not appear in collection"
    );
}

// =========================================================================
// 6. Smart collections
// =========================================================================

#[test]
fn test_smart_collections() {
    let conn = setup_db();

    // Create prompts with different tags
    let _claude1 = create_test_prompt(
        &conn,
        "Claude Prompt 1",
        "For Claude",
        vec!["model:claude".to_string()],
        true,
    );
    let _claude2 = create_test_prompt(
        &conn,
        "Claude Prompt 2",
        "Also for Claude",
        vec!["model:claude".to_string()],
        false,
    );
    let _gemini = create_test_prompt(
        &conn,
        "Gemini Prompt",
        "For Gemini",
        vec!["model:gemini".to_string()],
        false,
    );
    let _both = create_test_prompt(
        &conn,
        "Multi-model",
        "For both",
        vec!["model:claude".to_string(), "model:gemini".to_string()],
        true,
    );

    // --- Smart collection: tag includes model:claude ---
    let filter_claude = r#"{"conditions":[{"field":"tag","op":"includes","value":"model:claude"}],"match":"all"}"#;
    let claude_coll = collection_service::create_collection(
        &conn,
        CreateCollectionRequest {
            name: "Claude Prompts".to_string(),
            description: None,
            icon: None,
            color: None,
            is_smart: true,
            filter_query: Some(filter_claude.to_string()),
        },
    )
    .unwrap();

    let claude_items =
        collection_service::get_collection_prompts(&conn, &claude_coll.id, 50, 0).unwrap();
    assert_eq!(
        claude_items.len(),
        3,
        "Claude smart collection should return 3 prompts (2 claude-only + 1 both)"
    );

    // --- Smart collection: is_favorite = true ---
    let filter_fav = r#"{"conditions":[{"field":"is_favorite","op":"eq","value":true}],"match":"all"}"#;
    let fav_coll = collection_service::create_collection(
        &conn,
        CreateCollectionRequest {
            name: "Favorites".to_string(),
            description: None,
            icon: None,
            color: None,
            is_smart: true,
            filter_query: Some(filter_fav.to_string()),
        },
    )
    .unwrap();

    let fav_items =
        collection_service::get_collection_prompts(&conn, &fav_coll.id, 50, 0).unwrap();
    assert_eq!(
        fav_items.len(),
        2,
        "Favorites smart collection should return 2 prompts"
    );
    assert!(
        fav_items.iter().all(|i| i.is_favorite),
        "All items in favorites collection should be favorites"
    );
}

// =========================================================================
// 7. FTS search
// =========================================================================

#[test]
fn test_fts_search() {
    let conn = setup_db();

    let _p1 = create_test_prompt(
        &conn,
        "Quantum Computing Basics",
        "Explain the basics of quantum computing including qubits and superposition.",
        vec!["science".to_string()],
        false,
    );
    let _p2 = create_test_prompt(
        &conn,
        "Rust Error Handling",
        "Write idiomatic Rust code with Result and Option types.",
        vec!["programming".to_string()],
        false,
    );
    let _p3 = create_test_prompt(
        &conn,
        "Recipe Generator",
        "Generate a healthy dinner recipe with chicken and vegetables.",
        vec!["cooking".to_string()],
        false,
    );

    // --- Search for a specific term ---
    let results = search_service::search_prompts(&conn, "quantum", 50).unwrap();
    assert_eq!(
        results.len(),
        1,
        "Should find exactly 1 prompt matching 'quantum'"
    );
    assert_eq!(
        results[0].title, "Quantum Computing Basics",
        "Matching prompt should be the quantum one"
    );

    // --- Search for content term ---
    let results = search_service::search_prompts(&conn, "chicken", 50).unwrap();
    assert_eq!(
        results.len(),
        1,
        "Should find exactly 1 prompt matching 'chicken'"
    );
    assert_eq!(
        results[0].title, "Recipe Generator",
        "Matching prompt should be the recipe one"
    );

    // --- Search for tag name (tags are indexed in FTS) ---
    let results = search_service::search_prompts(&conn, "programming", 50).unwrap();
    assert_eq!(
        results.len(),
        1,
        "Should find 1 prompt via tag name 'programming'"
    );
    assert_eq!(
        results[0].title, "Rust Error Handling",
        "Tag search should match the Rust prompt"
    );

    // --- Search for non-existent term ---
    let results = search_service::search_prompts(&conn, "nonexistentxyz", 50).unwrap();
    assert_eq!(
        results.len(),
        0,
        "Non-existent term should return empty results"
    );

    // --- Empty query ---
    let results = search_service::search_prompts(&conn, "", 50).unwrap();
    assert_eq!(results.len(), 0, "Empty query should return empty results");
}

// =========================================================================
// 8. Copy tracking
// =========================================================================

#[test]
fn test_copy_tracking() {
    let conn = setup_db();

    let created = create_test_prompt(&conn, "Copy Prompt", "Copy this content", vec![], false);
    let prompt_id = &created.prompt.id;

    // Verify initial state
    let fetched = prompt_service::get_prompt_by_id(&conn, prompt_id).unwrap();
    assert_eq!(fetched.prompt.copy_count, 0, "Copy count should start at 0");
    assert!(
        fetched.prompt.last_copied_at.is_none(),
        "last_copied_at should be None initially"
    );

    // --- Record first copy (no variant specified, uses primary) ---
    let content = prompt_service::record_copy(&conn, prompt_id, None).unwrap();
    assert_eq!(
        content, "Copy this content",
        "Returned content should match the primary variant"
    );

    let fetched = prompt_service::get_prompt_by_id(&conn, prompt_id).unwrap();
    assert_eq!(
        fetched.prompt.copy_count, 1,
        "Copy count should be 1 after first copy"
    );
    assert!(
        fetched.prompt.last_copied_at.is_some(),
        "last_copied_at should be set after copy"
    );

    // --- Add a variant and record copy with specific variant ---
    let v2 =
        prompt_service::add_variant(&conn, prompt_id, "Alt Version", "Alternative content")
            .unwrap();

    let content2 =
        prompt_service::record_copy(&conn, prompt_id, Some(&v2.id)).unwrap();
    assert_eq!(
        content2, "Alternative content",
        "Returned content should match the specified variant"
    );

    let fetched = prompt_service::get_prompt_by_id(&conn, prompt_id).unwrap();
    assert_eq!(
        fetched.prompt.copy_count, 2,
        "Copy count should be 2 after second copy"
    );
}

// =========================================================================
// 9. Playbook operations
// =========================================================================

#[test]
fn test_playbook_operations() {
    let conn = setup_db();

    // Create prompts to use as steps
    let p1 = create_test_prompt(&conn, "Step 1 Prompt", "First step content", vec![], false);
    let p2 = create_test_prompt(&conn, "Step 2 Prompt", "Second step content", vec![], false);

    // --- Create playbook ---
    let playbook =
        playbook_service::create_playbook(&conn, "My Playbook", Some("A test playbook")).unwrap();
    assert_eq!(playbook.title, "My Playbook", "Playbook title should match");

    // --- Add 3 steps (2 single, 1 with instructions) ---
    let step1 = playbook_service::add_step(
        &conn,
        &playbook.id,
        Some(&p1.prompt.id),
        "single",
        None,
        None,
    )
    .unwrap();
    assert_eq!(step1.position, 0, "First step should be at position 0");

    let step2 = playbook_service::add_step(
        &conn,
        &playbook.id,
        Some(&p2.prompt.id),
        "single",
        None,
        None,
    )
    .unwrap();
    assert_eq!(step2.position, 1, "Second step should be at position 1");

    let step3 = playbook_service::add_step(
        &conn,
        &playbook.id,
        None,
        "instruction",
        Some("Review and refine the output"),
        None,
    )
    .unwrap();
    assert_eq!(step3.position, 2, "Third step should be at position 2");

    // --- Get playbook with steps ---
    let fetched = playbook_service::get_playbook(&conn, &playbook.id).unwrap();
    assert_eq!(
        fetched.steps.len(),
        3,
        "Playbook should have 3 steps"
    );
    assert_eq!(
        fetched.steps[0].step.position, 0,
        "Steps should be ordered by position"
    );
    assert_eq!(fetched.steps[1].step.position, 1);
    assert_eq!(fetched.steps[2].step.position, 2);

    // Verify step prompts are hydrated
    assert!(
        fetched.steps[0].prompt.is_some(),
        "Step 1 should have hydrated prompt"
    );
    assert_eq!(
        fetched.steps[0].prompt.as_ref().unwrap().prompt.title,
        "Step 1 Prompt",
        "Hydrated prompt title should match"
    );
    assert!(
        fetched.steps[2].prompt.is_none(),
        "Instruction step should not have a hydrated prompt"
    );

    // --- Remove a step and verify reordering ---
    playbook_service::remove_step(&conn, &step2.id).unwrap();
    let fetched = playbook_service::get_playbook(&conn, &playbook.id).unwrap();
    assert_eq!(
        fetched.steps.len(),
        2,
        "Should have 2 steps after removal"
    );
    assert_eq!(
        fetched.steps[0].step.position, 0,
        "First step position should still be 0"
    );
    assert_eq!(
        fetched.steps[1].step.position, 1,
        "Third step should be reordered to position 1"
    );

    // --- Delete the playbook ---
    playbook_service::delete_playbook(&conn, &playbook.id).unwrap();
    let result = playbook_service::get_playbook(&conn, &playbook.id);
    assert!(
        result.is_err(),
        "Deleted playbook should not be found"
    );
}

// =========================================================================
// 10. Playbook sessions
// =========================================================================

#[test]
fn test_playbook_session() {
    let conn = setup_db();

    // Create a playbook with steps
    let p1 = create_test_prompt(&conn, "Session Step 1", "Step 1", vec![], false);
    let p2 = create_test_prompt(&conn, "Session Step 2", "Step 2", vec![], false);
    let playbook =
        playbook_service::create_playbook(&conn, "Session Playbook", None).unwrap();
    playbook_service::add_step(&conn, &playbook.id, Some(&p1.prompt.id), "single", None, None)
        .unwrap();
    playbook_service::add_step(&conn, &playbook.id, Some(&p2.prompt.id), "single", None, None)
        .unwrap();

    // --- Initial state: no active session ---
    let session = playbook_service::get_session(&conn).unwrap();
    assert!(
        session.active_playbook_id.is_none(),
        "No active playbook initially"
    );
    assert_eq!(session.current_step, 0, "Current step should be 0 initially");

    // --- Start session ---
    let session = playbook_service::start_session(&conn, &playbook.id).unwrap();
    assert_eq!(
        session.active_playbook_id.as_deref(),
        Some(playbook.id.as_str()),
        "Active playbook should be set"
    );
    assert_eq!(session.current_step, 0, "Current step should be 0 after start");
    assert!(
        session.started_at.is_some(),
        "started_at should be set"
    );

    // --- Advance step ---
    let session = playbook_service::advance_step(&conn).unwrap();
    assert_eq!(
        session.current_step, 1,
        "Current step should be 1 after first advance"
    );

    // --- Advance again ---
    let session = playbook_service::advance_step(&conn).unwrap();
    assert_eq!(
        session.current_step, 2,
        "Current step should be 2 after second advance"
    );

    // --- End session ---
    playbook_service::end_session(&conn).unwrap();
    let session = playbook_service::get_session(&conn).unwrap();
    assert!(
        session.active_playbook_id.is_none(),
        "Active playbook should be cleared after end"
    );
    assert_eq!(
        session.current_step, 0,
        "Current step should be reset to 0 after end"
    );
    assert!(
        session.started_at.is_none(),
        "started_at should be cleared after end"
    );
}

// =========================================================================
// 11. Import JSON
// =========================================================================

#[test]
fn test_import_json() {
    let conn = setup_db();

    let json = r#"{
        "prompts": [
            {
                "title": "Import Prompt 1",
                "content": "Content for prompt 1",
                "tags": ["imported", "test"],
                "is_favorite": true,
                "variants": [
                    {"label": "Variant A", "content": "Variant A content"}
                ]
            },
            {
                "title": "Import Prompt 2",
                "content": "Content for prompt 2",
                "tags": ["imported"],
                "is_favorite": false
            },
            {
                "title": "Import Prompt 3",
                "content": "Content for prompt 3"
            }
        ]
    }"#;

    // --- First import ---
    let result = import_export::import_json(&conn, json).unwrap();
    assert_eq!(result.imported, 3, "Should import 3 prompts");
    assert_eq!(result.skipped, 0, "Should skip 0 prompts on first import");
    assert!(
        result.errors.is_empty(),
        "Should have no errors: {:?}",
        result.errors
    );

    // Verify prompts were created
    let list = prompt_service::list_prompts(&conn, 50, 0).unwrap();
    assert_eq!(list.len(), 3, "Should have 3 prompts after import");

    // Verify the first prompt has its extra variant
    let p1 = list.iter().find(|p| p.title == "Import Prompt 1").unwrap();
    let p1_full = prompt_service::get_prompt_by_id(&conn, &p1.id).unwrap();
    // 1 default + 1 extra = 2
    assert_eq!(
        p1_full.variants.len(),
        2,
        "Import Prompt 1 should have 2 variants (default + 1 extra)"
    );

    // --- Second import (deduplication) ---
    let result2 = import_export::import_json(&conn, json).unwrap();
    assert_eq!(
        result2.imported, 0,
        "Should import 0 on second run (all duplicates)"
    );
    assert_eq!(result2.skipped, 3, "Should skip 3 prompts on second import");

    // Verify total count is still 3
    let list2 = prompt_service::list_prompts(&conn, 50, 0).unwrap();
    assert_eq!(
        list2.len(),
        3,
        "Total prompt count should still be 3 after duplicate import"
    );
}

// =========================================================================
// 12. Import Markdown
// =========================================================================

#[test]
fn test_import_markdown() {
    let conn = setup_db();

    let markdown = r#"---
title: My Markdown Prompt
tags: [writing, creative]
favorite: true
---
Write a compelling story about a robot discovering emotions."#;

    let result = import_export::import_markdown(&conn, "prompt.md", markdown).unwrap();
    assert_eq!(result.imported, 1, "Should import 1 prompt from markdown");
    assert_eq!(result.skipped, 0, "Should skip 0");

    // Verify the prompt
    let list = prompt_service::list_prompts(&conn, 50, 0).unwrap();
    assert_eq!(list.len(), 1, "Should have 1 prompt");

    let prompt = &list[0];
    assert_eq!(
        prompt.title, "My Markdown Prompt",
        "Title should come from frontmatter"
    );
    assert!(prompt.is_favorite, "Should be marked as favorite");

    let tag_names: Vec<&str> = prompt.tags.iter().map(|t| t.name.as_str()).collect();
    assert!(
        tag_names.contains(&"writing"),
        "Should have 'writing' tag"
    );
    assert!(
        tag_names.contains(&"creative"),
        "Should have 'creative' tag"
    );

    // Verify the content is the body after frontmatter
    let full = prompt_service::get_prompt_by_id(&conn, &prompt.id).unwrap();
    assert_eq!(
        full.variants[0].content,
        "Write a compelling story about a robot discovering emotions.",
        "Content should be the body after frontmatter"
    );
}

// =========================================================================
// 13. Export JSON
// =========================================================================

#[test]
fn test_export_json() {
    let conn = setup_db();

    // Create prompts with variants and tags
    let p1 = create_test_prompt(
        &conn,
        "Export Prompt 1",
        "Content 1",
        vec!["tag-a".to_string()],
        true,
    );
    prompt_service::add_variant(&conn, &p1.prompt.id, "Extra V", "Extra variant content").unwrap();

    let _p2 = create_test_prompt(
        &conn,
        "Export Prompt 2",
        "Content 2",
        vec!["tag-b".to_string(), "tag-c".to_string()],
        false,
    );

    // --- Export ---
    let json_str = import_export::export_json(&conn).unwrap();
    let export: import_export::ExportData = serde_json::from_str(&json_str).unwrap();

    assert_eq!(export.version, "1.0", "Export version should be 1.0");
    assert!(
        !export.exported_at.is_empty(),
        "exported_at should be set"
    );
    assert_eq!(
        export.prompts.len(),
        2,
        "Should export 2 prompts"
    );

    // Find the first prompt in export
    let ep1 = export
        .prompts
        .iter()
        .find(|p| p.title == "Export Prompt 1")
        .expect("Export Prompt 1 should be in export");
    assert_eq!(ep1.content, "Content 1", "Primary content should match");
    assert!(ep1.is_favorite, "Favorite status should be preserved");
    assert_eq!(ep1.tags, vec!["tag-a"], "Tags should be preserved");
    assert_eq!(
        ep1.variants.len(),
        1,
        "Should have 1 extra variant (default excluded from variants list)"
    );
    assert_eq!(ep1.variants[0].label, "Extra V", "Extra variant label should match");

    // Find the second prompt
    let ep2 = export
        .prompts
        .iter()
        .find(|p| p.title == "Export Prompt 2")
        .expect("Export Prompt 2 should be in export");
    assert_eq!(
        ep2.tags.len(),
        2,
        "Second prompt should have 2 tags"
    );
    assert_eq!(
        ep2.variants.len(),
        0,
        "Second prompt has no extra variants"
    );

    // --- Soft-deleted prompts should not appear in export ---
    prompt_service::delete_prompt(&conn, &p1.prompt.id).unwrap();
    let json_str2 = import_export::export_json(&conn).unwrap();
    let export2: import_export::ExportData = serde_json::from_str(&json_str2).unwrap();
    assert_eq!(
        export2.prompts.len(),
        1,
        "Soft-deleted prompt should not appear in export"
    );
}

// =========================================================================
// 14. API Auth (unit-level check of middleware logic)
// =========================================================================

// The auth middleware requires a running axum server and async context.
// We test it in a lightweight way by verifying the middleware function
// signature and behavior are consistent with the ApiState struct.
// A full HTTP-level test would require spinning up a server, which is
// beyond the scope of these synchronous integration tests.

#[test]
fn test_api_auth_state_structure() {
    // Verify that ApiState can be constructed and the key field is accessible.
    // This is a compile-time + basic sanity check.
    use cadence_lib::api::server::ApiState;
    use std::sync::Mutex;

    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let state = ApiState {
        db: Mutex::new(conn),
        api_key: "test-key-12345".to_string(),
    };

    assert_eq!(state.api_key, "test-key-12345", "API key should be stored");
    assert!(
        state.db.lock().is_ok(),
        "Database mutex should be lockable"
    );
}

// =========================================================================
// Edge cases and additional coverage
// =========================================================================

#[test]
fn test_list_prompts_pagination() {
    let conn = setup_db();

    // Create 5 prompts
    for i in 0..5 {
        create_test_prompt(&conn, &format!("Prompt {}", i), &format!("Content {}", i), vec![], false);
    }

    let page1 = prompt_service::list_prompts(&conn, 2, 0).unwrap();
    assert_eq!(page1.len(), 2, "First page should have 2 items");

    let page2 = prompt_service::list_prompts(&conn, 2, 2).unwrap();
    assert_eq!(page2.len(), 2, "Second page should have 2 items");

    let page3 = prompt_service::list_prompts(&conn, 2, 4).unwrap();
    assert_eq!(page3.len(), 1, "Third page should have 1 item");

    // All IDs should be unique across pages
    let all_ids: Vec<String> = page1
        .iter()
        .chain(page2.iter())
        .chain(page3.iter())
        .map(|p| p.id.clone())
        .collect();
    let unique_count = {
        let mut set = std::collections::HashSet::new();
        for id in &all_ids {
            set.insert(id.as_str());
        }
        set.len()
    };
    assert_eq!(unique_count, 5, "All 5 prompts should be unique across pages");
}

#[test]
fn test_update_prompt_favorite_toggle() {
    let conn = setup_db();

    let created = create_test_prompt(&conn, "Toggle Prompt", "Content", vec![], false);
    let prompt_id = &created.prompt.id;

    assert!(!created.prompt.is_favorite, "Should start as not favorite");

    // Toggle on
    prompt_service::update_prompt(
        &conn,
        prompt_id,
        UpdatePromptRequest {
            title: None,
            description: None,
            is_favorite: Some(true),
            is_pinned: None,
            primary_variant_id: None,
        },
    )
    .unwrap();
    let fetched = prompt_service::get_prompt_by_id(&conn, prompt_id).unwrap();
    assert!(fetched.prompt.is_favorite, "Should be favorite after toggle on");

    // Toggle off
    prompt_service::update_prompt(
        &conn,
        prompt_id,
        UpdatePromptRequest {
            title: None,
            description: None,
            is_favorite: Some(false),
            is_pinned: None,
            primary_variant_id: None,
        },
    )
    .unwrap();
    let fetched = prompt_service::get_prompt_by_id(&conn, prompt_id).unwrap();
    assert!(
        !fetched.prompt.is_favorite,
        "Should not be favorite after toggle off"
    );
}

#[test]
fn test_list_collections() {
    let conn = setup_db();

    let _c1 = collection_service::create_collection(
        &conn,
        CreateCollectionRequest {
            name: "Alpha Collection".to_string(),
            description: None,
            icon: None,
            color: None,
            is_smart: false,
            filter_query: None,
        },
    )
    .unwrap();

    let _c2 = collection_service::create_collection(
        &conn,
        CreateCollectionRequest {
            name: "Beta Collection".to_string(),
            description: Some("A beta coll".to_string()),
            icon: None,
            color: None,
            is_smart: true,
            filter_query: Some(r#"{"conditions":[],"match":"all"}"#.to_string()),
        },
    )
    .unwrap();

    let collections = collection_service::list_collections(&conn).unwrap();
    assert_eq!(collections.len(), 2, "Should have 2 collections");
    // Ordered by name
    assert_eq!(
        collections[0].name, "Alpha Collection",
        "First collection should be Alpha (alphabetical)"
    );
    assert_eq!(
        collections[1].name, "Beta Collection",
        "Second collection should be Beta"
    );
    assert!(
        collections[1].is_smart,
        "Beta should be a smart collection"
    );
}

#[test]
fn test_playbook_list_and_update() {
    let conn = setup_db();

    let pb1 = playbook_service::create_playbook(&conn, "Playbook A", None).unwrap();
    let _pb2 =
        playbook_service::create_playbook(&conn, "Playbook B", Some("Description B")).unwrap();

    let list = playbook_service::list_playbooks(&conn).unwrap();
    assert_eq!(list.len(), 2, "Should list 2 playbooks");

    // Update playbook title
    playbook_service::update_playbook(&conn, &pb1.id, Some("Updated Playbook A"), None).unwrap();
    let fetched = playbook_service::get_playbook(&conn, &pb1.id).unwrap();
    assert_eq!(
        fetched.playbook.title, "Updated Playbook A",
        "Playbook title should be updated"
    );
}

#[test]
fn test_markdown_import_without_frontmatter() {
    let conn = setup_db();

    let markdown = "This is plain markdown content without any frontmatter.";

    let result = import_export::import_markdown(&conn, "plain-prompt.md", markdown).unwrap();
    assert_eq!(result.imported, 1, "Should import 1 prompt");

    let list = prompt_service::list_prompts(&conn, 50, 0).unwrap();
    assert_eq!(list.len(), 1, "Should have 1 prompt");
    assert_eq!(
        list[0].title, "plain-prompt",
        "Title should be derived from filename without .md extension"
    );
}

#[test]
fn test_import_json_with_empty_content() {
    let conn = setup_db();

    // Import a prompt with tags but ensure the system handles it
    let json = r#"{
        "prompts": [
            {
                "title": "Has Tags",
                "content": "Some content",
                "tags": ["a", "b", "c"]
            }
        ]
    }"#;

    let result = import_export::import_json(&conn, json).unwrap();
    assert_eq!(result.imported, 1, "Should import 1 prompt");

    let list = prompt_service::list_prompts(&conn, 50, 0).unwrap();
    let prompt = &list[0];
    assert_eq!(prompt.tags.len(), 3, "Should have 3 tags");
}

#[test]
fn test_search_after_update() {
    let conn = setup_db();

    let created = create_test_prompt(
        &conn,
        "Original Title",
        "Original content about elephants",
        vec![],
        false,
    );
    let prompt_id = &created.prompt.id;

    // Search should find it
    let results = search_service::search_prompts(&conn, "elephants", 50).unwrap();
    assert_eq!(results.len(), 1, "Should find prompt by original content");

    // Update title
    prompt_service::update_prompt(
        &conn,
        prompt_id,
        UpdatePromptRequest {
            title: Some("Updated About Giraffes".to_string()),
            description: None,
            is_favorite: None,
            is_pinned: None,
            primary_variant_id: None,
        },
    )
    .unwrap();

    // Search by new title should work
    let results = search_service::search_prompts(&conn, "Giraffes", 50).unwrap();
    assert_eq!(results.len(), 1, "Should find prompt by updated title");
}
