use serde::{Deserialize, Serialize};

/// A collection record from the `collections` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub is_smart: bool,
    pub filter_query: Option<String>,
    pub sort_field: Option<String>,
    pub sort_order: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Request body for creating a new collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCollectionRequest {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub is_smart: bool,
    pub filter_query: Option<String>,
}
