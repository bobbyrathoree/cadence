use cadence_core::error::{AppError, AppResult};
use cadence_core::models::playbook::{PlaybookStepWithPrompt, PlaybookWithSteps};
use cadence_core::models::prompt::{PromptWithVariants, Variant};
use cadence_core::services::playbook_service::PlaybookCountRow;
use cadence_core::services::prompt_service::SummaryRow;
use cadence_core::services::tag_service::TagCountRow;
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PromptSummary {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub updated_at: Option<String>,
    pub snippet: Option<String>,
}

impl From<SummaryRow> for PromptSummary {
    fn from(row: SummaryRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            description: row.description,
            tags: row.tags,
            is_favorite: row.is_favorite,
            is_pinned: row.is_pinned,
            updated_at: row.updated_at,
            snippet: row.snippet,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct VariantOut {
    pub id: String,
    pub label: Option<String>,
    pub content: String,
    pub is_primary: bool,
}

impl VariantOut {
    fn from_variant(variant: Variant, primary_variant_id: Option<&str>) -> Self {
        Self {
            is_primary: primary_variant_id == Some(variant.id.as_str()),
            id: variant.id,
            label: Some(variant.label),
            content: variant.content,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PromptDetailOut {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub primary_variant_id: Option<String>,
    pub variants: Vec<VariantOut>,
}

impl From<PromptWithVariants> for PromptDetailOut {
    fn from(prompt: PromptWithVariants) -> Self {
        let primary_variant_id = prompt.prompt.primary_variant_id;
        Self {
            id: prompt.prompt.id,
            title: prompt.prompt.title,
            description: prompt.prompt.description,
            tags: prompt.tags.into_iter().map(|tag| tag.name).collect(),
            is_favorite: prompt.prompt.is_favorite,
            is_pinned: prompt.prompt.is_pinned,
            created_at: prompt.prompt.created_at,
            updated_at: prompt.prompt.updated_at,
            variants: prompt
                .variants
                .into_iter()
                .map(|variant| VariantOut::from_variant(variant, primary_variant_id.as_deref()))
                .collect(),
            primary_variant_id,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PlaybookSummary {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub step_count: u32,
}

impl TryFrom<PlaybookCountRow> for PlaybookSummary {
    type Error = AppError;

    fn try_from(row: PlaybookCountRow) -> AppResult<Self> {
        Ok(Self {
            id: row.id,
            title: row.title,
            description: row.description,
            step_count: checked_u32(row.step_count, "playbook step count")?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct StepOut {
    pub position: u32,
    pub step_type: String,
    pub instructions: Option<String>,
    pub prompt: Option<PromptDetailOut>,
    pub choice_prompts: Vec<PromptDetailOut>,
    pub missing: bool,
}

impl TryFrom<PlaybookStepWithPrompt> for StepOut {
    type Error = AppError;

    fn try_from(step: PlaybookStepWithPrompt) -> AppResult<Self> {
        let missing = match step.step.step_type.as_str() {
            "choice" => step.choice_prompts.len() < 2,
            _ => step.prompt.is_none(),
        };
        Ok(Self {
            position: checked_u32(step.step.position, "playbook step position")?,
            step_type: step.step.step_type,
            instructions: step.step.instructions,
            prompt: step.prompt.map(PromptDetailOut::from),
            choice_prompts: step
                .choice_prompts
                .into_iter()
                .map(PromptDetailOut::from)
                .collect(),
            missing,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PlaybookDetailOut {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub steps: Vec<StepOut>,
}

impl TryFrom<PlaybookWithSteps> for PlaybookDetailOut {
    type Error = AppError;

    fn try_from(playbook: PlaybookWithSteps) -> AppResult<Self> {
        Ok(Self {
            id: playbook.playbook.id,
            title: playbook.playbook.title,
            description: playbook.playbook.description,
            steps: playbook
                .steps
                .into_iter()
                .map(StepOut::try_from)
                .collect::<AppResult<Vec<_>>>()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TagOut {
    pub name: String,
    pub color: Option<String>,
    pub prompt_count: u32,
}

impl TryFrom<TagCountRow> for TagOut {
    type Error = AppError;

    fn try_from(row: TagCountRow) -> AppResult<Self> {
        Ok(Self {
            name: row.name,
            color: row.color,
            prompt_count: checked_u32(row.prompt_count, "tag prompt count")?,
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct SearchArgs {
    pub query: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PageOut {
    pub prompts: Vec<PromptSummary>,
    pub next_offset: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ListArgs {
    pub filter: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct GetArgs {
    pub id_or_title: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TagsOut {
    pub tags: Vec<TagOut>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PlaybooksOut {
    pub playbooks: Vec<PlaybookSummary>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct RecordCopyArgs {
    pub prompt_id: String,
    pub variant_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct RecordCopyOut {
    pub content: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct CreatePromptArgs {
    pub title: String,
    pub content: String,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub variant_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct UpdateContentArgs {
    pub prompt_id: String,
    pub variant_id: Option<String>,
    pub content: String,
}

fn checked_u32(value: i64, label: &str) -> AppResult<u32> {
    u32::try_from(value)
        .map_err(|_| AppError::internal(format!("{label} is outside the supported range")))
}
