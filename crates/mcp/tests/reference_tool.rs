use cadence_core::db::{schema, Db, Health};
use cadence_core::db_access::DbAccess;
use cadence_core::models::prompt::CreatePromptRequest;
use cadence_core::services::prompt_service;
use cadence_mcp::{enable_query_only, CadenceMcp};
use rmcp::model::CallToolRequestParams;
use rmcp::ServiceExt;
use serde_json::json;

fn database() -> (Db, Health) {
    let (health, _messages) = Health::recording();
    let db = Db {
        conn: rusqlite::Connection::open_in_memory().expect("open in-memory database"),
        health: health.clone(),
    };
    schema::create_tables(&db.conn).expect("create schema");
    (db, health)
}

#[tokio::test]
async fn reference_echo_title_round_trips_through_real_serve() {
    let (mut db, health) = database();
    let prompt = prompt_service::create_prompt(
        &mut db,
        CreatePromptRequest {
            title: "Reference prompt".to_string(),
            description: None,
            content: "Reference content".to_string(),
            variant_label: None,
            tags: Vec::new(),
            is_favorite: false,
        },
    )
    .expect("create prompt");
    let prompt_id = prompt.prompt.id;
    enable_query_only(&mut db).expect("enable query-only mode");
    let server = CadenceMcp::with_reference_router(DbAccess::new(db), health);
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server_task = tokio::spawn(async move {
        server
            .serve(server_transport)
            .await
            .expect("serve reference server")
            .waiting()
            .await
            .expect("wait for reference server");
    });
    let client = ().serve(client_transport).await.expect("connect client");
    let mut arguments = serde_json::Map::new();
    arguments.insert("id_or_title".to_string(), json!(prompt_id.clone()));
    let result = client
        .call_tool(CallToolRequestParams::new("echo_title").with_arguments(arguments))
        .await
        .expect("call echo_title");

    assert_eq!(
        result.structured_content,
        Some(json!({
            "id": prompt_id,
            "title": "Reference prompt"
        }))
    );
    client.cancel().await.expect("cancel client");
    server_task.await.expect("join server task");
}

#[tokio::test]
async fn production_server_lists_exactly_nine_tools() {
    let (mut db, health) = database();
    enable_query_only(&mut db).expect("enable query-only mode");
    let server = CadenceMcp::for_test(DbAccess::new(db), health, true);
    let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);

    let server_task = tokio::spawn(async move {
        server
            .serve(server_transport)
            .await
            .expect("serve production server")
            .waiting()
            .await
            .expect("wait for production server");
    });
    let client = ().serve(client_transport).await.expect("connect client");
    let names = client
        .list_all_tools()
        .await
        .expect("list production tools")
        .into_iter()
        .map(|tool| tool.name.into_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        [
            "create_prompt",
            "get_playbook",
            "get_prompt",
            "list_playbooks",
            "list_prompts",
            "list_tags",
            "record_copy",
            "search_prompts",
            "update_prompt_content",
        ]
    );
    client.cancel().await.expect("cancel client");
    server_task.await.expect("join server task");
}
