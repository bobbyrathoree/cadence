use serde::{Deserialize, Serialize};

/// A tag record from the `tags` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: Option<String>,
}

/// Request body for creating a new tag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub color: Option<String>,
}
