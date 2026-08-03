use std::borrow::Cow;

use cadence_core::db::{Db, Health};
use cadence_core::db_access::DbAccess;
use cadence_core::error::{AppError, AppResult};
use cadence_core::services::transaction::UNRECOVERABLE_DB_MSG;
use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::ServerHandler;

pub const SERVER_INSTRUCTIONS: &str = "Cadence is a local prompt library. Use search_prompts/get_prompt to find and read saved prompts, list_playbooks/get_playbook to read multi-step prompt workflows (follow steps in order; 'instructions' fields are operator guidance), and record_copy to fetch a prompt's text when you use it. Prompts may contain {{variable}} placeholders to fill in.";

pub const SUPPORTED_PROTOCOL_VERSIONS: &[ProtocolVersion] =
    &[ProtocolVersion::V_2025_11_25, ProtocolVersion::V_2026_07_28];

#[derive(Clone)]
pub struct McpState {
    db: DbAccess,
    health: Health,
}

impl McpState {
    pub fn new(db: DbAccess, health: Health) -> Self {
        Self { db, health }
    }

    pub async fn blocking_db<T, F>(&self, operation: F) -> AppResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Db) -> AppResult<T> + Send + 'static,
    {
        if self.health.is_poisoned() {
            return Err(AppError::internal(UNRECOVERABLE_DB_MSG));
        }

        let health = self.health.clone();
        self.db
            .with_async(move |db| {
                if health.is_poisoned() {
                    return Err(AppError::internal(UNRECOVERABLE_DB_MSG));
                }
                operation(db)
            })
            .await
    }
}

impl ServerHandler for McpState {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .enable_resources()
                .enable_completions()
                .build(),
        )
        .with_server_info(Implementation::new(
            "cadence-mcp",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_instructions(SERVER_INSTRUCTIONS)
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(SUPPORTED_PROTOCOL_VERSIONS)
    }
}
