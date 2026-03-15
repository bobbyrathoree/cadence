use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::models::prompt::CreatePromptRequest;
use crate::services::prompt_service;

// ------------------------------------------------------------------
// Data types
// ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportData {
    pub version: String,
    pub exported_at: String,
    pub prompts: Vec<ExportPrompt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPrompt {
    pub title: String,
    pub description: Option<String>,
    pub content: String,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub variants: Vec<ExportVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportVariant {
    pub label: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPromptData {
    pub title: String,
    pub description: Option<String>,
    pub content: String,
    pub tags: Option<Vec<String>>,
    pub is_favorite: Option<bool>,
    pub variants: Option<Vec<ExportVariant>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportData {
    pub prompts: Vec<ImportPromptData>,
}

// ------------------------------------------------------------------
// Deduplication helper
// ------------------------------------------------------------------

/// Check whether a prompt with the same title AND similar content (first 200 chars) already exists.
fn is_duplicate(conn: &Connection, title: &str, content: &str) -> bool {
    let content_prefix: String = content.chars().take(200).collect();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prompts p
             JOIN variants v ON v.id = p.primary_variant_id AND v.deleted_at IS NULL
             WHERE p.title = ?1 AND SUBSTR(v.content, 1, 200) = ?2 AND p.deleted_at IS NULL",
            params![title, content_prefix],
            |row| row.get(0),
        )
        .unwrap_or(0);

    count > 0
}

// ------------------------------------------------------------------
// JSON import / export
// ------------------------------------------------------------------

/// Import prompts from a JSON string. Expects `ImportData` format (object with `prompts` array).
pub fn import_json(conn: &Connection, json_str: &str) -> rusqlite::Result<ImportResult> {
    let data: ImportData = serde_json::from_str(json_str).map_err(|e| {
        rusqlite::Error::InvalidParameterName(format!("Invalid JSON: {}", e))
    })?;

    let mut result = ImportResult {
        imported: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    for (i, prompt_data) in data.prompts.into_iter().enumerate() {
        // Deduplication check
        if is_duplicate(conn, &prompt_data.title, &prompt_data.content) {
            result.skipped += 1;
            continue;
        }

        let req = CreatePromptRequest {
            title: prompt_data.title.clone(),
            description: prompt_data.description,
            content: prompt_data.content,
            variant_label: None,
            tags: prompt_data.tags.unwrap_or_default(),
            is_favorite: prompt_data.is_favorite.unwrap_or(false),
        };

        match prompt_service::create_prompt(conn, req) {
            Ok(created) => {
                // Add extra variants beyond the default one
                if let Some(variants) = prompt_data.variants {
                    for variant in variants {
                        if let Err(e) =
                            prompt_service::add_variant(conn, &created.prompt.id, &variant.label, &variant.content)
                        {
                            result.errors.push(format!(
                                "Prompt #{} ({}): failed to add variant '{}': {}",
                                i, prompt_data.title, variant.label, e
                            ));
                        }
                    }
                }
                result.imported += 1;
            }
            Err(e) => {
                result.errors.push(format!(
                    "Prompt #{} ({}): {}",
                    i, prompt_data.title, e
                ));
            }
        }
    }

    Ok(result)
}

/// Export all non-deleted prompts as a pretty-printed JSON string.
pub fn export_json(conn: &Connection) -> rusqlite::Result<String> {
    // Get all non-deleted prompt IDs
    let mut stmt = conn.prepare(
        "SELECT id FROM prompts WHERE deleted_at IS NULL ORDER BY updated_at DESC",
    )?;
    let ids: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut prompts = Vec::new();
    for id in ids {
        let pwv = prompt_service::get_prompt_by_id(conn, &id)?;

        // The first variant is the primary/default one; extra variants are everything else
        let mut extra_variants = Vec::new();
        let mut primary_content = String::new();

        for variant in &pwv.variants {
            if Some(&variant.id) == pwv.prompt.primary_variant_id.as_ref() {
                primary_content = variant.content.clone();
            } else {
                extra_variants.push(ExportVariant {
                    label: variant.label.clone(),
                    content: variant.content.clone(),
                });
            }
        }

        // If no primary variant matched (shouldn't happen), use first variant content
        if primary_content.is_empty() && !pwv.variants.is_empty() {
            primary_content = pwv.variants[0].content.clone();
        }

        prompts.push(ExportPrompt {
            title: pwv.prompt.title,
            description: pwv.prompt.description,
            content: primary_content,
            tags: pwv.tags.iter().map(|t| t.name.clone()).collect(),
            is_favorite: pwv.prompt.is_favorite,
            variants: extra_variants,
        });
    }

    let export_data = ExportData {
        version: "1.0".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        prompts,
    };

    serde_json::to_string_pretty(&export_data).map_err(|e| {
        rusqlite::Error::InvalidParameterName(format!("JSON serialization error: {}", e))
    })
}

// ------------------------------------------------------------------
// Markdown import
// ------------------------------------------------------------------

/// Parse simple YAML frontmatter from markdown content.
/// Returns (title, tags, favorite, body).
fn parse_markdown_frontmatter(
    filename: &str,
    content: &str,
) -> (String, Vec<String>, bool, String) {
    let trimmed = content.trim();

    // Check if it starts with frontmatter delimiter
    if !trimmed.starts_with("---") {
        // No frontmatter — use filename as title, entire content as body
        let title = filename
            .trim_end_matches(".md")
            .trim_end_matches(".markdown")
            .to_string();
        return (title, Vec::new(), false, content.to_string());
    }

    // Find the second `---` delimiter
    let after_first = &trimmed[3..];
    let second_delimiter = after_first.find("---");

    let (frontmatter_str, body) = match second_delimiter {
        Some(pos) => {
            let fm = &after_first[..pos];
            let body = &after_first[pos + 3..];
            (fm.trim(), body.trim())
        }
        None => {
            // No closing delimiter — treat entire content as body
            let title = filename
                .trim_end_matches(".md")
                .trim_end_matches(".markdown")
                .to_string();
            return (title, Vec::new(), false, content.to_string());
        }
    };

    // Parse frontmatter fields line by line
    let mut title: Option<String> = None;
    let mut tags: Vec<String> = Vec::new();
    let mut favorite = false;

    for line in frontmatter_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix("title:") {
            title = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(rest) = line.strip_prefix("tags:") {
            tags = parse_yaml_list(rest.trim());
        } else if let Some(rest) = line.strip_prefix("favorite:") {
            let val = rest.trim().to_lowercase();
            favorite = val == "true" || val == "yes";
        }
    }

    // Fall back to filename if no title in frontmatter
    let title = title.unwrap_or_else(|| {
        filename
            .trim_end_matches(".md")
            .trim_end_matches(".markdown")
            .to_string()
    });

    (title, tags, favorite, body.to_string())
}

/// Parse a simple YAML inline list like `[tag1, tag2]` or `tag1, tag2`.
fn parse_yaml_list(s: &str) -> Vec<String> {
    let s = s.trim();
    // Remove surrounding brackets if present
    let s = if s.starts_with('[') && s.ends_with(']') {
        &s[1..s.len() - 1]
    } else {
        s
    };

    s.split(',')
        .map(|item| item.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

/// Import a single markdown file. Returns an ImportResult for that file.
pub fn import_markdown(
    conn: &Connection,
    filename: &str,
    content: &str,
) -> rusqlite::Result<ImportResult> {
    let (title, tags, favorite, body) = parse_markdown_frontmatter(filename, content);

    let mut result = ImportResult {
        imported: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    if body.trim().is_empty() {
        result.errors.push(format!("{}: empty content", filename));
        return Ok(result);
    }

    // Deduplication check
    if is_duplicate(conn, &title, &body) {
        result.skipped += 1;
        return Ok(result);
    }

    let req = CreatePromptRequest {
        title,
        description: None,
        content: body,
        variant_label: None,
        tags,
        is_favorite: favorite,
    };

    match prompt_service::create_prompt(conn, req) {
        Ok(_) => {
            result.imported += 1;
        }
        Err(e) => {
            result.errors.push(format!("{}: {}", filename, e));
        }
    }

    Ok(result)
}

/// Import multiple markdown files. Aggregates results from each file.
pub fn import_markdown_batch(
    conn: &Connection,
    files: Vec<(String, String)>,
) -> rusqlite::Result<ImportResult> {
    let mut aggregate = ImportResult {
        imported: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    for (filename, content) in files {
        match import_markdown(conn, &filename, &content) {
            Ok(r) => {
                aggregate.imported += r.imported;
                aggregate.skipped += r.skipped;
                aggregate.errors.extend(r.errors);
            }
            Err(e) => {
                aggregate.errors.push(format!("{}: {}", filename, e));
            }
        }
    }

    Ok(aggregate)
}
