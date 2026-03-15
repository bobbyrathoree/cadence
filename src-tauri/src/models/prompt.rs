use serde::{Deserialize, Serialize};

use super::tag::Tag;

/// A prompt record from the `prompts` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub primary_variant_id: Option<String>,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub copy_count: i64,
    pub last_copied_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub deleted_at: Option<String>,
}

/// A variant record from the `variants` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    pub id: String,
    pub prompt_id: String,
    pub label: String,
    pub content: String,
    pub content_type: Option<String>,
    pub variables: Option<String>,
    pub sort_order: i64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub deleted_at: Option<String>,
}

/// A prompt together with its variants and tags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptWithVariants {
    #[serde(flatten)]
    pub prompt: Prompt,
    pub variants: Vec<Variant>,
    pub tags: Vec<Tag>,
}

/// Request body for creating a new prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePromptRequest {
    pub title: String,
    pub description: Option<String>,
    pub content: String,
    pub variant_label: Option<String>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
}

/// Request body for updating an existing prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePromptRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub is_favorite: Option<bool>,
    pub is_pinned: Option<bool>,
    pub primary_variant_id: Option<String>,
}

/// Lightweight prompt representation for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptListItem {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub snippet: String,
    pub is_favorite: bool,
    pub variant_count: i64,
    pub copy_count: i64,
    pub last_copied_at: Option<String>,
    pub tags: Vec<Tag>,
}
