use cadence_core::db::{schema, Db, Health};
use cadence_core::models::prompt::CreatePromptRequest;
use cadence_core::services::{playbook_service, prompt_service};
use rusqlite::{params, Connection};

fn database() -> Db {
    let db = Db {
        conn: Connection::open_in_memory().expect("open in-memory database"),
        health: Health::exit_process(1),
    };
    schema::create_tables(&db.conn).expect("create schema");
    db
}

fn create_prompt(db: &mut Db, title: &str) -> String {
    prompt_service::create_prompt(
        db,
        CreatePromptRequest {
            title: title.to_string(),
            description: None,
            content: format!("{title} content"),
            variant_label: None,
            tags: Vec::new(),
            is_favorite: false,
        },
    )
    .expect("create prompt")
    .prompt
    .id
}

#[test]
fn prompt_completion_treats_like_metacharacters_as_literals() {
    let mut db = database();
    let percent = create_prompt(&mut db, "% literal");
    let underscore = create_prompt(&mut db, "_ literal");
    let slash = create_prompt(&mut db, r"\ literal");
    create_prompt(&mut db, "ordinary");

    assert_eq!(
        prompt_service::complete_prompt_ids(&db.conn, "%", 10).expect("complete percent"),
        [percent]
    );
    assert_eq!(
        prompt_service::complete_prompt_ids(&db.conn, "_", 10).expect("complete underscore"),
        [underscore]
    );
    assert_eq!(
        prompt_service::complete_prompt_ids(&db.conn, r"\", 10).expect("complete slash"),
        [slash]
    );
}

#[test]
fn playbook_completion_treats_like_metacharacters_as_literals() {
    let db = database();
    for (id, title) in [
        ("10000000-0000-4000-8000-000000000001", "% literal"),
        ("10000000-0000-4000-8000-000000000002", "_ literal"),
        ("10000000-0000-4000-8000-000000000003", r"\ literal"),
        ("10000000-0000-4000-8000-000000000004", "ordinary"),
    ] {
        db.conn
            .execute(
                "INSERT INTO playbooks (id, title) VALUES (?1, ?2)",
                params![id, title],
            )
            .expect("insert playbook");
    }

    assert_eq!(
        playbook_service::complete_playbook_ids(&db.conn, "%", 10).expect("complete percent"),
        ["10000000-0000-4000-8000-000000000001"]
    );
    assert_eq!(
        playbook_service::complete_playbook_ids(&db.conn, "_", 10).expect("complete underscore"),
        ["10000000-0000-4000-8000-000000000002"]
    );
    assert_eq!(
        playbook_service::complete_playbook_ids(&db.conn, r"\", 10).expect("complete slash"),
        ["10000000-0000-4000-8000-000000000003"]
    );
}

#[test]
fn prompt_variants_use_the_pinned_tie_break_order() {
    let mut db = database();
    let prompt_id = create_prompt(&mut db, "Ordered variants");
    db.conn
        .execute(
            "INSERT INTO variants
                (id, prompt_id, label, content, sort_order, created_at)
             VALUES
                ('bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', ?1, 'B', 'B', 0, '2026-01-02'),
                ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', ?1, 'A', 'A', 0, '2026-01-02'),
                ('cccccccc-cccc-4ccc-8ccc-cccccccccccc', ?1, 'C', 'C', 0, '2026-01-01')",
            params![prompt_id],
        )
        .expect("insert tied variants");

    let prompt =
        prompt_service::get_prompt_by_id(&db.conn, &prompt_id).expect("hydrate prompt variants");
    let tied_ids = prompt
        .variants
        .into_iter()
        .filter(|variant| variant.id != prompt.prompt.primary_variant_id.as_deref().unwrap_or(""))
        .map(|variant| variant.id)
        .collect::<Vec<_>>();
    assert_eq!(
        tied_ids,
        [
            "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
        ]
    );
}
