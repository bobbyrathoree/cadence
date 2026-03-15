use serde::{Deserialize, Serialize};

/// A copy-history record from the `copy_history` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyHistory {
    pub id: String,
    pub prompt_id: String,
    pub variant_id: Option<String>,
    pub copied_at: Option<String>,
    pub metadata: Option<String>,
}
