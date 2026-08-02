use rusqlite::{params, Connection};

use crate::error::AppResult;
use crate::models::prompt::{PromptListItem, SnippetRun};
use crate::services::tag_service;

/// Search prompts using an allowlisted FTS5 query.
/// Tokens are implicitly ANDed, with prefix matching on the final token only.
/// Returns PromptListItem with Unicode-safe, pre-segmented snippet runs.
///
/// Uses `fts_mapping` to join FTS results back to prompts via a simple
/// SQL JOIN, avoiding the previous O(N) hash-all-IDs reverse lookup.
pub fn search_prompts(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> AppResult<Vec<PromptListItem>> {
    let tokens = tokenize_query(query);
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    let last_index = tokens.len() - 1;
    let fts_query = tokens
        .iter()
        .enumerate()
        .map(|(index, token)| {
            if index == last_index {
                format!("\"{token}\"*")
            } else {
                format!("\"{token}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(" AND ");

    // Single query: join FTS results through fts_mapping to prompts
    let mut stmt = conn.prepare(
        "SELECT p.id, p.title, p.description, p.is_favorite, p.copy_count,
                p.last_copied_at,
                COALESCE(v.content, '') AS primary_content,
                (SELECT COUNT(*) FROM variants WHERE prompt_id = p.id AND deleted_at IS NULL) AS variant_count
         FROM prompts_fts f
         JOIN fts_mapping m ON m.rowid = f.rowid
         JOIN prompts p ON p.id = m.prompt_id
         LEFT JOIN variants v
           ON v.id = p.primary_variant_id
          AND v.prompt_id = p.id
          AND v.deleted_at IS NULL
         WHERE prompts_fts MATCH ?1 AND p.deleted_at IS NULL
         ORDER BY f.rank
         LIMIT ?2",
    )?;

    let rows = stmt.query_map(params![fts_query, limit], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, i64>(3)? != 0,
            row.get::<_, i64>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, i64>(7)?,
        ))
    })?;

    let mut items = Vec::new();
    for row in rows {
        let (
            id,
            title,
            description,
            is_favorite,
            copy_count,
            last_copied_at,
            primary_content,
            variant_count,
        ) = row?;
        let tags = tag_service::get_tags_for_prompt(conn, &id)?;
        let snippet_runs = build_snippet_runs(&primary_content, &tokens);
        let snippet = snippet_runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        items.push(PromptListItem {
            id,
            title,
            description,
            snippet,
            snippet_runs,
            is_favorite,
            variant_count,
            copy_count,
            last_copied_at,
            tags,
        });
    }

    Ok(items)
}

fn tokenize_query(query: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for character in query.chars() {
        if character.is_alphanumeric() || character == '_' {
            current.push(character);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn build_snippet_runs(content: &str, query_tokens: &[String]) -> Vec<SnippetRun> {
    let characters = content.chars().collect::<Vec<_>>();
    if characters.is_empty() {
        return Vec::new();
    }

    let normalized_tokens = query_tokens
        .iter()
        .map(|token| token.to_lowercase())
        .collect::<Vec<_>>();
    let spans = text_token_spans(&characters);
    let first_match = spans
        .iter()
        .find(|span| token_matches(&span.normalized, &normalized_tokens));
    let (window_start, window_end) = match first_match {
        Some(span) => (
            span.start.saturating_sub(60),
            (span.end + 60).min(characters.len()),
        ),
        None => (0, characters.len().min(120)),
    };

    let mut highlighted = vec![false; window_end - window_start];
    for span in spans
        .iter()
        .filter(|span| token_matches(&span.normalized, &normalized_tokens))
    {
        let start = span.start.max(window_start);
        let end = span.end.min(window_end);
        for position in start..end {
            highlighted[position - window_start] = true;
        }
    }

    let mut runs = Vec::new();
    for (offset, character) in characters[window_start..window_end].iter().enumerate() {
        let is_highlighted = highlighted[offset];
        match runs.last_mut() {
            Some(SnippetRun { text, highlighted }) if *highlighted == is_highlighted => {
                text.push(*character)
            }
            _ => runs.push(SnippetRun {
                text: character.to_string(),
                highlighted: is_highlighted,
            }),
        }
    }
    runs
}

struct TextTokenSpan {
    start: usize,
    end: usize,
    normalized: String,
}

fn text_token_spans(characters: &[char]) -> Vec<TextTokenSpan> {
    let mut spans = Vec::new();
    let mut start = None;

    for (index, character) in characters.iter().copied().enumerate() {
        if character.is_alphanumeric() || character == '_' {
            start.get_or_insert(index);
        } else if let Some(start) = start.take() {
            spans.push(TextTokenSpan {
                start,
                end: index,
                normalized: characters[start..index]
                    .iter()
                    .collect::<String>()
                    .to_lowercase(),
            });
        }
    }
    if let Some(start) = start {
        spans.push(TextTokenSpan {
            start,
            end: characters.len(),
            normalized: characters[start..]
                .iter()
                .collect::<String>()
                .to_lowercase(),
        });
    }

    spans
}

fn token_matches(candidate: &str, query_tokens: &[String]) -> bool {
    query_tokens.iter().enumerate().any(|(index, token)| {
        if index + 1 == query_tokens.len() {
            candidate.starts_with(token)
        } else {
            candidate == token
        }
    })
}
