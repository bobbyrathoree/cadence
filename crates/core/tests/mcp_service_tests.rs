use cadence_core::db::{schema, Db, Health};
use cadence_core::models::prompt::CreatePromptRequest;
use cadence_core::services::{
    playbook_service,
    prompt_service::{self, PromptFilter},
    tag_service,
};
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

fn ids(rows: &[prompt_service::SummaryRow]) -> Vec<&str> {
    rows.iter().map(|row| row.id.as_str()).collect()
}

#[test]
fn prompt_summary_queries_filter_soft_deletes_and_use_pinned_ordering() {
    let mut db = database();
    let first = create_prompt(&mut db, "Duplicate");
    let second = create_prompt(&mut db, "Duplicate");
    let null_updated = create_prompt(&mut db, "Duplicate");
    let deleted = create_prompt(&mut db, "Duplicate");
    db.conn
        .execute(
            "UPDATE prompts
             SET updated_at = CASE id
                    WHEN ?1 THEN '2026-02-01'
                    WHEN ?2 THEN '2026-02-01'
                    WHEN ?3 THEN NULL
                    WHEN ?4 THEN '2027-01-01'
                 END,
                 is_favorite = CASE WHEN id IN (?1, ?3, ?4) THEN 1 ELSE 0 END,
                 last_copied_at = CASE id
                    WHEN ?1 THEN '2026-01-01'
                    WHEN ?2 THEN '2026-03-01'
                    WHEN ?4 THEN '2027-01-01'
                    ELSE NULL
                 END,
                 deleted_at = CASE WHEN id = ?4 THEN '2026-08-03' ELSE NULL END",
            params![first, second, null_updated, deleted],
        )
        .expect("configure summary rows");

    let mut tied = [first.as_str(), second.as_str()];
    tied.sort_unstable_by(|left, right| right.cmp(left));
    let expected_all = [tied[0], tied[1], null_updated.as_str()];
    let all = prompt_service::list_prompt_summaries(&db.conn, PromptFilter::All, 10, 0)
        .expect("list all summaries");
    assert_eq!(ids(&all), expected_all);

    let favorites = prompt_service::list_prompt_summaries(&db.conn, PromptFilter::Favorites, 10, 0)
        .expect("list favorite summaries");
    assert_eq!(ids(&favorites), [first.as_str(), null_updated.as_str()]);

    let recent = prompt_service::list_prompt_summaries(&db.conn, PromptFilter::Recent, 10, 0)
        .expect("list recent summaries");
    assert_eq!(ids(&recent), [second.as_str(), first.as_str()]);

    let exact = prompt_service::find_prompts_by_exact_title(&db.conn, "Duplicate")
        .expect("find exact prompt titles");
    assert_eq!(ids(&exact), expected_all);
}

#[test]
fn prompt_catalogs_filter_soft_deletes_and_use_title_then_id_ordering() {
    let mut db = database();
    let alpha_a = create_prompt(&mut db, "Alpha");
    let alpha_b = create_prompt(&mut db, "Alpha");
    let favorite = create_prompt(&mut db, "Bravo");
    let excluded = create_prompt(&mut db, "Charlie");
    let deleted = create_prompt(&mut db, "Before");
    db.conn
        .execute(
            "UPDATE prompts
             SET is_pinned = CASE WHEN id IN (?1, ?2, ?5) THEN 1 ELSE 0 END,
                 is_favorite = CASE WHEN id IN (?3, ?5) THEN 1 ELSE 0 END,
                 deleted_at = CASE WHEN id = ?5 THEN '2026-08-03' ELSE NULL END
             WHERE id IN (?1, ?2, ?3, ?4, ?5)",
            params![alpha_a, alpha_b, favorite, excluded, deleted],
        )
        .expect("configure catalog rows");

    let mut alphas = [alpha_a.as_str(), alpha_b.as_str()];
    alphas.sort_unstable();
    let agent = prompt_service::agent_catalog(&db.conn, 10).expect("load agent catalog");
    assert_eq!(
        agent
            .iter()
            .map(|prompt| prompt.prompt.id.as_str())
            .collect::<Vec<_>>(),
        [alphas[0], alphas[1], favorite.as_str()]
    );
    let pinned = prompt_service::pinned_catalog(&db.conn, 10).expect("load pinned catalog");
    assert_eq!(
        pinned
            .iter()
            .map(|prompt| prompt.prompt.id.as_str())
            .collect::<Vec<_>>(),
        alphas
    );
}

#[test]
fn search_and_content_updates_refresh_fts_for_primary_and_explicit_variants() {
    let mut db = database();
    let prompt_id = create_prompt(&mut db, "Searchable");
    let primary_id = prompt_service::get_prompt_by_id(&db.conn, &prompt_id)
        .expect("load primary")
        .prompt
        .primary_variant_id
        .expect("primary variant id");
    let alternate =
        prompt_service::add_variant(&mut db, &prompt_id, "Alternate", "alternate original")
            .expect("add alternate");

    assert_eq!(
        ids(
            &prompt_service::search_prompt_summaries(&db.conn, "Searchable content", 10, 0)
                .expect("search original primary")
        ),
        [prompt_id.as_str()]
    );
    let primary = prompt_service::update_prompt_content(
        &mut db,
        &prompt_id,
        None,
        "primary replacement needle",
    )
    .expect("update primary content");
    assert_eq!(
        primary
            .variants
            .iter()
            .find(|variant| variant.id == primary_id)
            .map(|variant| variant.content.as_str()),
        Some("primary replacement needle")
    );
    assert!(
        prompt_service::search_prompt_summaries(&db.conn, "Searchable content", 10, 0)
            .expect("search removed primary content")
            .is_empty()
    );
    assert_eq!(
        ids(
            &prompt_service::search_prompt_summaries(&db.conn, "replacement needle", 10, 0)
                .expect("search replacement primary")
        ),
        [prompt_id.as_str()]
    );

    let updated = prompt_service::update_prompt_content(
        &mut db,
        &prompt_id,
        Some(&alternate.id),
        "alternate replacement",
    )
    .expect("update explicit variant");
    assert_eq!(
        updated
            .variants
            .iter()
            .find(|variant| variant.id == alternate.id)
            .map(|variant| variant.content.as_str()),
        Some("alternate replacement")
    );
    assert_eq!(
        ids(
            &prompt_service::search_prompt_summaries(&db.conn, "replacement needle", 10, 0)
                .expect("primary remains indexed")
        ),
        [prompt_id.as_str()]
    );
}

#[test]
fn search_summaries_filter_soft_deletes_and_break_rank_ties_by_id() {
    let mut db = database();
    let first = create_prompt(&mut db, "Rank tie");
    let second = create_prompt(&mut db, "Rank tie");
    let deleted = create_prompt(&mut db, "Ghost result");
    db.conn
        .execute(
            "UPDATE variants SET content = 'tie needle'
             WHERE prompt_id IN (?1, ?2)",
            params![first, second],
        )
        .expect("configure tied search content");
    for id in [&first, &second] {
        prompt_service::update_prompt_content(&mut db, id, None, "tie needle")
            .expect("reindex tied search content");
    }
    db.conn
        .execute(
            "UPDATE prompts SET deleted_at = '2026-08-03' WHERE id = ?1",
            params![deleted],
        )
        .expect("soft-delete indexed search row");

    let mut expected = [first.as_str(), second.as_str()];
    expected.sort_unstable();
    let tied = prompt_service::search_prompt_summaries(&db.conn, "tie needle", 10, 0)
        .expect("search tied rows");
    assert_eq!(ids(&tied), expected);
    assert!(
        prompt_service::search_prompt_summaries(&db.conn, "Ghost result", 10, 0)
            .expect("search soft-deleted row")
            .is_empty()
    );
}

#[test]
fn playbook_and_tag_counts_use_pinned_ordering_and_live_prompts_only() {
    let mut db = database();
    let live = create_prompt(&mut db, "Live");
    let deleted = create_prompt(&mut db, "Deleted");
    db.conn
        .execute(
            "UPDATE prompts SET deleted_at = '2026-08-03' WHERE id = ?1",
            params![deleted],
        )
        .expect("soft-delete tagged prompt");
    db.conn
        .execute_batch(
            "INSERT INTO tags (id, name, color) VALUES
                ('tag-z', 'Zulu', NULL),
                ('tag-a', 'Alpha', '#fff');
             INSERT INTO playbooks (id, title, description) VALUES
                ('playbook-b', 'Beta', NULL),
                ('playbook-a2', 'Alpha', 'second'),
                ('playbook-a1', 'Alpha', 'first');
             INSERT INTO playbook_steps
                (id, playbook_id, position, step_type)
             VALUES
                ('step-1', 'playbook-b', 0, 'single'),
                ('step-2', 'playbook-b', 1, 'single'),
                ('step-3', 'playbook-a1', 0, 'single');",
        )
        .expect("insert count fixtures");
    db.conn
        .execute(
            "INSERT INTO prompt_tags (prompt_id, tag_id) VALUES
                (?1, 'tag-a'), (?2, 'tag-a'), (?2, 'tag-z')",
            params![live, deleted],
        )
        .expect("tag live and deleted prompts");

    let playbooks =
        playbook_service::list_playbooks_with_counts(&db.conn).expect("list playbook counts");
    assert_eq!(
        playbooks
            .iter()
            .map(|row| (row.id.as_str(), row.step_count))
            .collect::<Vec<_>>(),
        [("playbook-a1", 1), ("playbook-a2", 0), ("playbook-b", 2),]
    );
    let exact = playbook_service::find_playbooks_by_exact_title(&db.conn, "Alpha")
        .expect("find exact playbook titles");
    assert_eq!(
        exact.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        ["playbook-a1", "playbook-a2"]
    );

    let tags = tag_service::list_tags_with_counts(&db.conn).expect("list tag counts");
    assert_eq!(
        tags.iter()
            .map(|row| (row.name.as_str(), row.prompt_count))
            .collect::<Vec<_>>(),
        [("Alpha", 1), ("Zulu", 0)]
    );
}
