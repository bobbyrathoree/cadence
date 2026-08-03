use cadence_core::db::Db;
use cadence_core::error::{AppError, AppResult};
use cadence_core::models::playbook::PlaybookWithSteps;
use cadence_core::models::prompt::PromptWithVariants;
use cadence_core::services::{playbook_service, prompt_service};
use rmcp::model::{Resource, ResourceContents, ResourceTemplate};

pub(crate) const PROMPT_TEMPLATE: &str = "cadence://prompt/{id}";
pub(crate) const PLAYBOOK_TEMPLATE: &str = "cadence://playbook/{id}";
const MARKDOWN: &str = "text/markdown";

pub(crate) enum ResourceId {
    Prompt(String),
    Playbook(String),
}

pub(crate) fn list(db: &mut Db) -> AppResult<Vec<Resource>> {
    Ok(prompt_service::pinned_catalog(&db.conn, 100)?
        .into_iter()
        .map(|prompt| {
            Resource::new(
                format!("cadence://prompt/{}", prompt.prompt.id),
                prompt.prompt.title,
            )
            .with_mime_type(MARKDOWN)
        })
        .collect())
}

pub(crate) fn templates() -> Vec<ResourceTemplate> {
    vec![
        ResourceTemplate::new(PROMPT_TEMPLATE, "cadence-prompt")
            .with_description(
                "A prompt from the Cadence library, rendered as markdown. {id} is the prompt UUID.",
            )
            .with_mime_type(MARKDOWN),
        ResourceTemplate::new(PLAYBOOK_TEMPLATE, "cadence-playbook")
            .with_description(
                "A playbook (ordered prompt workflow) from Cadence, rendered as markdown. {id} is the playbook UUID.",
            )
            .with_mime_type(MARKDOWN),
    ]
}

pub(crate) fn parse_uri(uri: &str) -> Option<ResourceId> {
    if let Some(id) = uri.strip_prefix("cadence://prompt/") {
        return canonical_uuid(id).map(ResourceId::Prompt);
    }
    if let Some(id) = uri.strip_prefix("cadence://playbook/") {
        return canonical_uuid(id).map(ResourceId::Playbook);
    }
    None
}

pub(crate) fn read(db: &mut Db, uri: &str, id: ResourceId) -> AppResult<ResourceContents> {
    let text = match id {
        ResourceId::Prompt(id) => {
            let prompt = prompt_service::get_prompt_by_id(&db.conn, &id)?;
            render_prompt(&prompt)
        }
        ResourceId::Playbook(id) => {
            let playbook = playbook_service::get_playbook(&db.conn, &id)?;
            render_playbook(&playbook)?
        }
    };
    Ok(ResourceContents::text(text, uri).with_mime_type(MARKDOWN))
}

pub(crate) fn canonical_uuid(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return None;
    }
    for (index, byte) in bytes.iter().copied().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if byte != b'-' {
                return None;
            }
        } else if !byte.is_ascii_hexdigit() {
            return None;
        }
    }
    uuid::Uuid::parse_str(value)
        .ok()
        .map(|uuid| uuid.hyphenated().to_string())
}

fn render_prompt(prompt: &PromptWithVariants) -> String {
    let mut output = format!("# {}\n", normalize(&prompt.prompt.title));
    if !prompt.tags.is_empty() {
        let tags = prompt
            .tags
            .iter()
            .map(|tag| normalize(&tag.name))
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!("\nTags: {tags}\n"));
    }
    if let Some(description) = &prompt.prompt.description {
        output.push_str(&format!("\n{}\n", normalize(description)));
    }
    for variant in &prompt.variants {
        let label = if variant.label.is_empty() {
            "Primary".to_string()
        } else {
            normalize(&variant.label)
        };
        let content = normalize(&variant.content);
        let fence = "`".repeat(longest_backtick_run(&content).saturating_add(1).max(3));
        output.push_str(&format!("\n## {label}\n\n{fence}\n{content}\n{fence}\n"));
    }
    one_trailing_lf(output)
}

fn render_playbook(playbook: &PlaybookWithSteps) -> AppResult<String> {
    let mut output = format!("# {}\n", normalize(&playbook.playbook.title));
    if let Some(description) = &playbook.playbook.description {
        output.push_str(&format!("\n{}\n", normalize(description)));
    }
    output.push('\n');
    for step in &playbook.steps {
        let number = step
            .step
            .position
            .checked_add(1)
            .ok_or_else(|| AppError::internal("playbook step position overflow"))?;
        if step.step.step_type == "choice" {
            if step.choice_prompts.len() < 2 {
                output.push_str(&format!("{number}. Choice: *(missing prompts)*\n"));
            } else {
                let titles = step
                    .choice_prompts
                    .iter()
                    .map(|prompt| normalize(&prompt.prompt.title))
                    .collect::<Vec<_>>()
                    .join(" | ");
                output.push_str(&format!("{number}. Choice: {titles}\n"));
            }
        } else {
            let title = step
                .prompt
                .as_ref()
                .map(|prompt| normalize(&prompt.prompt.title))
                .unwrap_or_else(|| "*(missing prompt)*".to_string());
            output.push_str(&format!("{number}. {title}\n"));
        }
        if let Some(instructions) = &step.step.instructions {
            for line in normalize(instructions).split('\n') {
                output.push_str(&format!("   > {line}\n"));
            }
        }
    }
    Ok(one_trailing_lf(output))
}

fn normalize(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

fn longest_backtick_run(value: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for character in value.chars() {
        if character == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

fn one_trailing_lf(mut value: String) -> String {
    while value.ends_with('\n') {
        value.pop();
    }
    value.push('\n');
    value
}
