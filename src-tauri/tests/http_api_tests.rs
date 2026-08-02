use std::sync::{Arc, Mutex};

use cadence_lib::api::server::{self, ApiState};
use cadence_lib::db::schema;
use cadence_lib::models::collection::CreateCollectionRequest;
use cadence_lib::models::prompt::CreatePromptRequest;
use cadence_lib::services::{collection_service, playbook_service, prompt_service, tag_service};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;

static HTTP_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct TestIds {
    prompt: String,
    second_prompt: String,
    foreign_variant: String,
    variant: String,
    tag: String,
    collection: String,
    playbook: String,
}

struct TestServer {
    port: u16,
    key: String,
    state: Arc<ApiState>,
    ids: TestIds,
    shutdown: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl TestServer {
    async fn start() -> Self {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        schema::create_tables(&conn).unwrap();

        let prompt = prompt_service::create_prompt(
            &mut conn,
            prompt_request("Matrix prompt", vec!["matrix-tag".to_string()]),
        )
        .unwrap();
        let variant = prompt_service::add_variant(
            &mut conn,
            &prompt.prompt.id,
            "Secondary",
            "secondary content",
        )
        .unwrap();
        let second_prompt =
            prompt_service::create_prompt(&mut conn, prompt_request("Second prompt", vec![]))
                .unwrap();
        let collection = collection_service::create_collection(
            &mut conn,
            CreateCollectionRequest {
                name: "Matrix collection".to_string(),
                description: None,
                icon: None,
                color: None,
                is_smart: false,
                filter_query: None,
            },
        )
        .unwrap();
        let playbook =
            playbook_service::create_playbook(&mut conn, "Matrix playbook", None).unwrap();
        let tag = tag_service::list_tags(&conn)
            .unwrap()
            .into_iter()
            .find(|tag| tag.name == "matrix-tag")
            .unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let key = "cad_test_key".to_string();
        let state = Arc::new(ApiState {
            db: Mutex::new(conn),
            api_key: key.clone(),
            api_port: port,
        });
        let (shutdown, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(server::start(listener, state.clone(), async move {
            let _ = shutdown_rx.await;
        }));

        Self {
            port,
            key,
            state,
            ids: TestIds {
                prompt: prompt.prompt.id,
                second_prompt: second_prompt.prompt.id,
                foreign_variant: second_prompt.variants[0].id.clone(),
                variant: variant.id,
                tag: tag.id,
                collection: collection.id,
                playbook: playbook.id,
            },
            shutdown,
            task,
        }
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        scheme: &str,
        token: &str,
        body: &str,
    ) -> String {
        raw_request(
            self.port,
            method,
            path,
            &format!("127.0.0.1:{}", self.port),
            Some((scheme, token)),
            body,
        )
        .await
    }

    async fn authenticated(&self, method: &str, path: &str, body: &str) -> String {
        self.request(method, path, "Bearer", &self.key, body).await
    }

    async fn stop(self) {
        let _ = self.shutdown.send(());
        self.task.await.unwrap().unwrap();
    }
}

fn prompt_request(title: &str, tags: Vec<String>) -> CreatePromptRequest {
    CreatePromptRequest {
        title: title.to_string(),
        description: None,
        content: format!("{title} content"),
        variant_label: None,
        tags,
        is_favorite: false,
    }
}

struct RouteCase {
    method: &'static str,
    real_path: String,
    missing_path: String,
    body: String,
    success: u16,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_dynamic_route_maps_success_not_found_and_bad_auth() {
    let _guard = HTTP_TEST_LOCK.lock().await;
    let server = TestServer::start().await;
    let ids = &server.ids;
    let cases = vec![
        RouteCase {
            method: "GET",
            real_path: format!("/api/v1/prompts/{}", ids.prompt),
            missing_path: "/api/v1/prompts/missing".to_string(),
            body: String::new(),
            success: 200,
        },
        RouteCase {
            method: "PUT",
            real_path: format!("/api/v1/prompts/{}", ids.prompt),
            missing_path: "/api/v1/prompts/missing".to_string(),
            body: r#"{"title":"Updated matrix"}"#.to_string(),
            success: 204,
        },
        RouteCase {
            method: "POST",
            real_path: format!("/api/v1/prompts/{}/variants", ids.prompt),
            missing_path: "/api/v1/prompts/missing/variants".to_string(),
            body: r#"{"label":"API variant","content":"api content"}"#.to_string(),
            success: 200,
        },
        RouteCase {
            method: "PUT",
            real_path: format!("/api/v1/variants/{}", ids.variant),
            missing_path: "/api/v1/variants/missing".to_string(),
            body: r#"{"content":"updated secondary","label":"Updated"}"#.to_string(),
            success: 204,
        },
        RouteCase {
            method: "DELETE",
            real_path: format!("/api/v1/variants/{}", ids.variant),
            missing_path: "/api/v1/variants/missing".to_string(),
            body: String::new(),
            success: 204,
        },
        RouteCase {
            method: "POST",
            real_path: format!("/api/v1/prompts/{}/tags", ids.prompt),
            missing_path: "/api/v1/prompts/missing/tags".to_string(),
            body: r#"{"tags":["api-tag"]}"#.to_string(),
            success: 200,
        },
        RouteCase {
            method: "DELETE",
            real_path: format!("/api/v1/prompts/{}/tags/{}", ids.prompt, ids.tag),
            missing_path: "/api/v1/prompts/missing/tags/missing".to_string(),
            body: String::new(),
            success: 204,
        },
        RouteCase {
            method: "GET",
            real_path: format!("/api/v1/collections/{}/prompts", ids.collection),
            missing_path: "/api/v1/collections/missing/prompts".to_string(),
            body: String::new(),
            success: 200,
        },
        RouteCase {
            method: "POST",
            real_path: format!("/api/v1/collections/{}/prompts", ids.collection),
            missing_path: "/api/v1/collections/missing/prompts".to_string(),
            body: format!(r#"{{"prompt_id":"{}"}}"#, ids.second_prompt),
            success: 204,
        },
        RouteCase {
            method: "POST",
            real_path: format!("/api/v1/prompts/{}/copy", ids.prompt),
            missing_path: "/api/v1/prompts/missing/copy".to_string(),
            body: "{}".to_string(),
            success: 200,
        },
        RouteCase {
            method: "GET",
            real_path: format!("/api/v1/playbooks/{}", ids.playbook),
            missing_path: "/api/v1/playbooks/missing".to_string(),
            body: String::new(),
            success: 200,
        },
        RouteCase {
            method: "DELETE",
            real_path: format!("/api/v1/playbooks/{}", ids.playbook),
            missing_path: "/api/v1/playbooks/missing".to_string(),
            body: String::new(),
            success: 204,
        },
        RouteCase {
            method: "DELETE",
            real_path: format!("/api/v1/prompts/{}", ids.prompt),
            missing_path: "/api/v1/prompts/missing".to_string(),
            body: String::new(),
            success: 204,
        },
    ];

    for case in cases {
        let unauthorized = server
            .request(
                case.method,
                &case.real_path,
                "Bearer",
                "wrong-token",
                &case.body,
            )
            .await;
        assert_status(&unauthorized, 401, case.method, &case.real_path);

        let missing = server
            .authenticated(case.method, &case.missing_path, &case.body)
            .await;
        assert_status(&missing, 404, case.method, &case.missing_path);

        let success = server
            .authenticated(case.method, &case.real_path, &case.body)
            .await;
        assert_status(&success, case.success, case.method, &case.real_path);
    }

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bearer_scheme_is_case_insensitive_but_token_is_case_sensitive() {
    let _guard = HTTP_TEST_LOCK.lock().await;
    let server = TestServer::start().await;
    for scheme in ["bearer", "Bearer", "BEARER"] {
        let response = server
            .request("GET", "/api/v1/prompts", scheme, &server.key, "")
            .await;
        assert_status(&response, 200, "GET", scheme);
    }
    let wrong_case = server
        .request(
            "GET",
            "/api/v1/prompts",
            "Bearer",
            &server.key.to_uppercase(),
            "",
        )
        .await;
    assert_status(&wrong_case, 401, "GET", "case-sensitive token");
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_validation_is_exact_and_precedes_health_bypass() {
    let _guard = HTTP_TEST_LOCK.lock().await;
    let server = TestServer::start().await;
    for host in [
        format!("127.0.0.1:{}", server.port),
        format!("localhost:{}", server.port),
    ] {
        let response = raw_request(server.port, "GET", "/api/v1/health", &host, None, "").await;
        assert_status(&response, 200, "GET", &host);
    }
    for host in [
        "127.0.0.1".to_string(),
        format!("LOCALHOST:{}", server.port),
        format!("localhost:{}x", server.port),
        format!("[::1]:{}", server.port),
    ] {
        let response = raw_request(server.port, "GET", "/api/v1/health", &host, None, "").await;
        assert_status(&response, 400, "GET", &host);
    }
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn body_larger_than_four_megabytes_is_rejected() {
    let _guard = HTTP_TEST_LOCK.lock().await;
    let server = TestServer::start().await;
    let body = format!(r#"{{"padding":"{}"}}"#, "x".repeat(4 * 1024 * 1024));
    let response = server.authenticated("POST", "/api/v1/import", &body).await;
    assert_status(&response, 413, "POST", "/api/v1/import");
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn record_copy_for_soft_deleted_prompt_maps_to_not_found() {
    let _guard = HTTP_TEST_LOCK.lock().await;
    let server = TestServer::start().await;
    {
        let mut conn = server.state.db.lock().unwrap();
        prompt_service::delete_prompt(&mut conn, &server.ids.prompt).unwrap();
    }

    let response = server
        .authenticated(
            "POST",
            &format!("/api/v1/prompts/{}/copy", server.ids.prompt),
            &format!(r#"{{"variant_id":"{}"}}"#, server.ids.variant),
        )
        .await;
    assert_status(
        &response,
        404,
        "POST",
        "record copy for soft-deleted prompt",
    );
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_and_conflict_errors_map_to_400_and_409_without_sql_details() {
    let _guard = HTTP_TEST_LOCK.lock().await;
    let server = TestServer::start().await;

    let invalid = server
        .authenticated(
            "PUT",
            &format!("/api/v1/prompts/{}", server.ids.prompt),
            &format!(
                r#"{{"primary_variant_id":"{}"}}"#,
                server.ids.foreign_variant
            ),
        )
        .await;
    assert_status(&invalid, 400, "PUT", "foreign primary variant");

    server
        .state
        .db
        .lock()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER secret_conflict
             BEFORE UPDATE OF title ON prompts
             BEGIN
               SELECT RAISE(ABORT, 'secret prompts schema detail');
             END;",
        )
        .unwrap();
    let conflict = server
        .authenticated(
            "PUT",
            &format!("/api/v1/prompts/{}", server.ids.prompt),
            r#"{"title":"conflict"}"#,
        )
        .await;
    assert_status(&conflict, 409, "PUT", "constraint conflict");
    assert!(!conflict.contains("secret prompts schema detail"));
    assert!(!conflict.contains("CREATE TRIGGER"));
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn internal_errors_have_sanitized_bodies() {
    let _guard = HTTP_TEST_LOCK.lock().await;
    let server = TestServer::start().await;
    server
        .state
        .db
        .lock()
        .unwrap()
        .execute_batch("DROP TABLE prompts")
        .unwrap();

    let response = server
        .authenticated("GET", &format!("/api/v1/prompts/{}", server.ids.prompt), "")
        .await;
    assert_status(&response, 500, "GET", "internal failure");
    let body = response.split("\r\n\r\n").nth(1).unwrap_or_default();
    assert_eq!(body, r#"{"error":"Internal server error"}"#);
    assert!(!body.contains("prompts"));
    assert!(!body.contains("SQL"));
    server.stop().await;
}

async fn raw_request(
    port: u16,
    method: &str,
    path: &str,
    host: &str,
    authorization: Option<(&str, &str)>,
    body: &str,
) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let auth_header = authorization
        .map(|(scheme, token)| format!("Authorization: {scheme} {token}\r\n"))
        .unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\n\
         {auth_header}Content-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(request.as_bytes()).await;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    String::from_utf8(response).unwrap()
}

fn assert_status(response: &str, expected: u16, method: &str, path: &str) {
    let status = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok());
    assert_eq!(
        status,
        Some(expected),
        "{method} {path} expected {expected}, response: {response}"
    );
}
