use std::borrow::Cow;

use cadence_core::db::{Db, Health};
use cadence_core::db_access::DbAccess;
use cadence_core::error::{AppError, AppResult};
use cadence_core::services::transaction::UNRECOVERABLE_DB_MSG;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool_handler, ServerHandler};

pub const SERVER_INSTRUCTIONS: &str = "Cadence is a local prompt library. Use search_prompts/get_prompt to find and read saved prompts, list_playbooks/get_playbook to read multi-step prompt workflows (follow steps in order; 'instructions' fields are operator guidance), and record_copy to fetch a prompt's text when you use it. Prompts may contain {{variable}} placeholders to fill in.";

pub const SUPPORTED_PROTOCOL_VERSIONS: &[ProtocolVersion] =
    &[ProtocolVersion::V_2025_11_25, ProtocolVersion::V_2026_07_28];

#[derive(Clone)]
pub struct CadenceMcp {
    db: DbAccess,
    health: Health,
    tool_router: ToolRouter<Self>,
}

pub type McpState = CadenceMcp;

impl CadenceMcp {
    pub fn new(db: DbAccess, health: Health) -> Self {
        Self::build(
            db,
            health,
            std::env::var("CADENCE_MCP_ALLOW_WRITES").as_deref() == Ok("1"),
        )
    }

    fn build(db: DbAccess, health: Health, allow_writes: bool) -> Self {
        let mut tool_router = Self::production_tool_router();
        if !allow_writes {
            tool_router.remove_route("create_prompt");
            tool_router.remove_route("update_prompt_content");
        }
        Self {
            db,
            health,
            tool_router,
        }
    }

    #[cfg(feature = "test-support")]
    pub fn with_reference_router(db: DbAccess, health: Health) -> Self {
        let mut server = Self::new(db, health);
        server.tool_router.merge(Self::reference_tool_router());
        server
    }

    #[cfg(feature = "test-support")]
    pub fn for_test(db: DbAccess, health: Health, allow_writes: bool) -> Self {
        Self::build(db, health, allow_writes)
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

#[tool_handler(router = self.tool_router)]
impl ServerHandler for CadenceMcp {
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

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
        Ok(rmcp::model::ListToolsResult {
            result_type: Some(rmcp::model::ResultType::COMPLETE),
            tools: self.tool_router.list_all(),
            meta: None,
            next_cursor: None,
            ttl_ms: Some(0),
            cache_scope: Some(rmcp::model::CacheScope::Private),
        })
    }
}
