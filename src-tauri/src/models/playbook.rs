use serde::{Deserialize, Serialize};

use super::prompt::Prompt;

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
    pub step_type: Option<String>,
    pub instructions: Option<String>,
    pub choice_prompt_ids: Option<String>,
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
    pub prompt: Option<Prompt>,
    pub choice_prompts: Option<Vec<Prompt>>,
}

/// The singleton session tracking playbook progress from `playbook_sessions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookSession {
    pub id: i64,
    pub active_playbook_id: Option<String>,
    pub current_step: i64,
    pub started_at: Option<String>,
}
