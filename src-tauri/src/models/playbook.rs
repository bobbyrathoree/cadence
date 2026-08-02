use serde::{Deserialize, Serialize};

use super::patch::PatchField;
use super::prompt::PromptWithVariants;

/// A playbook record from the `playbooks` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playbook {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// A step within a playbook from the `playbook_steps` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookStep {
    pub id: String,
    pub playbook_id: String,
    pub prompt_id: Option<String>,
    pub position: i64,
    pub step_type: String,
    pub instructions: Option<String>,
    pub choice_prompt_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepSpec {
    pub step_type: String,
    pub prompt_id: Option<String>,
    #[serde(default)]
    pub choice_prompt_ids: Vec<String>,
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdatePlaybookRequest {
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "PatchField::is_keep")]
    pub description: PatchField<String>,
}

/// A playbook together with its steps (each enriched with prompt data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookWithSteps {
    #[serde(flatten)]
    pub playbook: Playbook,
    pub steps: Vec<PlaybookStepWithPrompt>,
}

/// A playbook step enriched with optional prompt data and choice prompts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookStepWithPrompt {
    #[serde(flatten)]
    pub step: PlaybookStep,
    pub prompt: Option<PromptWithVariants>,
    pub choice_prompts: Vec<PromptWithVariants>,
}

/// The singleton session tracking playbook progress from `playbook_sessions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookSession {
    pub id: i64,
    pub active_playbook_id: Option<String>,
    pub current_step: i64,
    pub started_at: Option<String>,
}
