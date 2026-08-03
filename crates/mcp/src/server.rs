use std::borrow::Cow;

use cadence_core::db::{Db, Health};
use cadence_core::db_access::DbAccess;
use cadence_core::error::{AppError, AppResult};
use cadence_core::services::transaction::UNRECOVERABLE_DB_MSG;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{
    CacheScope, CompleteRequestParams, CompleteResult, CompletionInfo, GetPromptRequestParams,
    GetPromptResponse, Implementation, ListPromptsResult, ListResourceTemplatesResult,
    ListResourcesResult, PaginatedRequestParams, ProtocolVersion, ReadResourceRequestParams,
    ReadResourceResponse, ReadResourceResult, Reference, ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::{tool_handler, ServerHandler};

use crate::tools::reference::REDACTED_INTERNAL;
use crate::{prompts, resources};

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

    fn ensure_healthy(&self) -> AppResult<()> {
        if self.health.is_poisoned() {
            Err(AppError::internal(UNRECOVERABLE_DB_MSG))
        } else {
            Ok(())
        }
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
        self.ensure_healthy().map_err(protocol_error)?;
        Ok(rmcp::model::ListToolsResult {
            result_type: Some(rmcp::model::ResultType::COMPLETE),
            tools: self.tool_router.list_all(),
            meta: None,
            next_cursor: None,
            ttl_ms: Some(0),
            cache_scope: Some(rmcp::model::CacheScope::Private),
        })
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<rmcp::RoleServer>,
    ) -> Result<ListPromptsResult, rmcp::ErrorData> {
        self.ensure_healthy().map_err(protocol_error)?;
        let prompts = self
            .blocking_db(prompts::list)
            .await
            .map_err(protocol_error)?;
        Ok(ListPromptsResult::with_all_items(prompts)
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<rmcp::RoleServer>,
    ) -> Result<GetPromptResponse, rmcp::ErrorData> {
        self.ensure_healthy().map_err(protocol_error)?;
        let result = self
            .blocking_db(move |db| prompts::get(db, &request.name, request.arguments))
            .await
            .map_err(prompt_error)?;
        Ok(result.into())
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<rmcp::RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        self.ensure_healthy().map_err(protocol_error)?;
        let resources = self
            .blocking_db(resources::list)
            .await
            .map_err(protocol_error)?;
        Ok(ListResourcesResult::with_all_items(resources)
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<rmcp::RoleServer>,
    ) -> Result<ListResourceTemplatesResult, rmcp::ErrorData> {
        self.ensure_healthy().map_err(protocol_error)?;
        Ok(
            ListResourceTemplatesResult::with_all_items(resources::templates())
                .with_ttl_ms(0)
                .with_cache_scope(CacheScope::Private),
        )
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<rmcp::RoleServer>,
    ) -> Result<ReadResourceResponse, rmcp::ErrorData> {
        self.ensure_healthy().map_err(protocol_error)?;
        let uri = request.uri;
        let id = resources::parse_uri(&uri).ok_or_else(|| {
            rmcp::ErrorData::invalid_params(format!("unknown resource: {uri}"), None)
        })?;
        let error_uri = uri.clone();
        let result = self
            .blocking_db(move |db| resources::read(db, &uri, id))
            .await
            .map_err(|error| resource_error(error, &error_uri))?;
        Ok(ReadResourceResult::new(vec![result])
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private)
            .into())
    }

    async fn complete(
        &self,
        request: CompleteRequestParams,
        _context: RequestContext<rmcp::RoleServer>,
    ) -> Result<CompleteResult, rmcp::ErrorData> {
        self.ensure_healthy().map_err(protocol_error)?;
        let template = match request.r#ref {
            Reference::Resource(reference) => reference.uri,
            _ => {
                return Err(rmcp::ErrorData::invalid_params(
                    "unknown completion target",
                    None,
                ));
            }
        };
        if template != resources::PROMPT_TEMPLATE && template != resources::PLAYBOOK_TEMPLATE {
            return Err(rmcp::ErrorData::invalid_params(
                "unknown completion target",
                None,
            ));
        }
        if request.argument.name != "id" {
            return completion_result(Vec::new(), false);
        }
        let prefix = request.argument.value;
        let prompt_target = template == resources::PROMPT_TEMPLATE;
        let mut values = self
            .blocking_db(move |db| {
                if prompt_target {
                    cadence_core::services::prompt_service::complete_prompt_ids(
                        &db.conn, &prefix, 10,
                    )
                } else {
                    cadence_core::services::playbook_service::complete_playbook_ids(
                        &db.conn, &prefix, 10,
                    )
                }
            })
            .await
            .map_err(protocol_error)?;
        let has_more = values.len() == 11;
        values.truncate(10);
        completion_result(values, has_more)
    }
}

fn completion_result(
    values: Vec<String>,
    has_more: bool,
) -> Result<CompleteResult, rmcp::ErrorData> {
    let completion = CompletionInfo::with_pagination(values, None, has_more)
        .map_err(|_| rmcp::ErrorData::internal_error(REDACTED_INTERNAL, None))?;
    Ok(CompleteResult::new(completion))
}

fn protocol_error(_error: AppError) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(REDACTED_INTERNAL, None)
}

fn prompt_error(error: AppError) -> rmcp::ErrorData {
    match error {
        AppError::NotFound => rmcp::ErrorData::invalid_params("prompt not found", None),
        other => protocol_error(other),
    }
}

fn resource_error(error: AppError, uri: &str) -> rmcp::ErrorData {
    match error {
        AppError::NotFound => {
            rmcp::ErrorData::resource_not_found(format!("resource not found: {uri}"), None)
        }
        other => protocol_error(other),
    }
}
