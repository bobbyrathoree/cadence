use cadence_core::error::AppError;
use rmcp::{
    handler::server::{common::schema_for_output, wrapper::Parameters},
    model::{CallToolResult, ContentBlock},
    schemars::{self, JsonSchema},
    tool, tool_router, ErrorData,
};
use serde::{Deserialize, Serialize};

pub const REDACTED_INTERNAL: &str = "An internal error occurred";

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct EchoTitleArgs {
    /// Prompt id or exact title
    pub id_or_title: String,
}
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct EchoTitleOut {
    pub id: String,
    pub title: String,
}

pub fn ok_structured<T: Serialize>(out: &T) -> Result<CallToolResult, ErrorData> {
    match serde_json::to_value(out) {
        // explicit match — no `?` into ErrorData
        Ok(v) => Ok(CallToolResult::structured(v)),
        Err(_) => Ok(CallToolResult::error(vec![ContentBlock::text(
            REDACTED_INTERNAL,
        )])),
    }
}
pub fn app_err(e: AppError) -> Result<CallToolResult, ErrorData> {
    let msg = match &e {
        AppError::Internal(_) => REDACTED_INTERNAL.to_string(),
        other => other.ipc_message(),
    };
    Ok(CallToolResult::error(vec![ContentBlock::text(msg)]))
}

#[tool_router(router = reference_tool_router, vis = "pub(crate)")] // R4: default visibility is private → E0624; pub(crate) + unique name required
impl super::CadenceMcp {
    /// Reference tool: resolve a prompt id/title to its id and title.
    #[tool(name = "echo_title",
           annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false),
           output_schema = schema_for_output::<EchoTitleOut>())]
    async fn echo_title(
        &self,
        Parameters(a): Parameters<EchoTitleArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .blocking_db(move |conn| super::resolve_prompt(conn, &a.id_or_title))
            .await
        {
            Ok(p) => ok_structured(&EchoTitleOut {
                id: p.id,
                title: p.title,
            }),
            Err(e) => app_err(e),
        }
    }
}
