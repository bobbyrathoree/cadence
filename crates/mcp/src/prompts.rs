use std::collections::HashMap;

use cadence_core::db::Db;
use cadence_core::error::{AppError, AppResult};
use cadence_core::models::prompt::PromptWithVariants;
use cadence_core::services::prompt_service;
use cadence_core::variables;
use rmcp::model::{GetPromptResult, JsonObject, Prompt, PromptArgument, PromptMessage, Role};
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

pub(crate) fn list(db: &mut Db) -> AppResult<Vec<Prompt>> {
    prompt_service::agent_catalog(&db.conn, 100)?
        .into_iter()
        .map(prompt_entry)
        .collect()
}

pub(crate) fn get(
    db: &mut Db,
    name: &str,
    arguments: Option<JsonObject>,
) -> AppResult<GetPromptResult> {
    let id = prompt_id_from_name(name).ok_or(AppError::NotFound)?;
    let prompt = prompt_service::get_prompt_by_id(&db.conn, &id)?;
    let content = primary_content(&prompt)?;
    let segments = variables::parse(content);
    let names = variables::variable_names(&segments);
    let mut values = HashMap::new();
    if let Some(arguments) = arguments {
        for name in names {
            if let Some(value) = arguments.get(&name).and_then(argument_value) {
                values.insert(name, value);
            }
        }
    }
    Ok(GetPromptResult::new(vec![PromptMessage::new_text(
        Role::User,
        variables::interpolate(&segments, &values),
    )]))
}

fn prompt_entry(prompt: PromptWithVariants) -> AppResult<Prompt> {
    let content = primary_content(&prompt)?;
    let arguments = variables::variable_names(&variables::parse(content))
        .into_iter()
        .map(|name| PromptArgument::new(name).with_required(false))
        .collect();
    Ok(Prompt::new(
        prompt_name(&prompt.prompt.title, &prompt.prompt.id),
        None::<String>,
        Some(arguments),
    ))
}

fn primary_content(prompt: &PromptWithVariants) -> AppResult<&str> {
    let primary_id = prompt
        .prompt
        .primary_variant_id
        .as_deref()
        .ok_or_else(|| AppError::internal("prompt primary variant is missing"))?;
    prompt
        .variants
        .iter()
        .find(|variant| variant.id == primary_id)
        .map(|variant| variant.content.as_str())
        .ok_or_else(|| AppError::internal("prompt primary variant is missing"))
}

fn argument_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

pub(crate) fn prompt_name(title: &str, id: &str) -> String {
    format!("{}-{id}", slugify(title))
}

fn prompt_id_from_name(name: &str) -> Option<String> {
    let tail = name.get(name.len().checked_sub(36)?..)?;
    super::resources::canonical_uuid(tail)
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in title.nfc().flat_map(char::to_lowercase) {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character);
            separator = false;
        } else {
            separator = true;
        }
    }
    slug.truncate(40);
    if slug.is_empty() {
        "p".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slug_is_normalized_ascii_bounded_and_nonempty() {
        assert_eq!(slugify("  HéLLo, WORLD!  "), "h-llo-world");
        assert_eq!(slugify("e\u{301}"), "p");
        assert_eq!(slugify("🚀"), "p");
        assert_eq!(slugify(&"a".repeat(50)), "a".repeat(40));
    }
}
