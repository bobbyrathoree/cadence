use cadence_core::db::Db;
use cadence_core::error::{AppError, AppResult};
use cadence_core::models::prompt::CreatePromptRequest;
use cadence_core::services::{
    playbook_service,
    prompt_service::{self, PromptFilter},
    tag_service,
};
use rmcp::{
    handler::server::{common::schema_for_output, wrapper::Parameters},
    model::CallToolResult,
    tool, tool_router, ErrorData,
};

use crate::dto::{
    CreatePromptArgs, GetArgs, ListArgs, PageOut, PlaybookDetailOut, PlaybookSummary, PlaybooksOut,
    PromptDetailOut, PromptSummary, RecordCopyArgs, RecordCopyOut, SearchArgs, TagOut, TagsOut,
    UpdateContentArgs,
};
use crate::fault;
use crate::write_scope::WriteScope;

use super::reference::{app_err, ok_structured};

#[tool_router(router = production_tool_router, vis = "pub(crate)")]
impl super::CadenceMcp {
    /// Create a new prompt in the Cadence library.
    #[tool(
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        ),
        output_schema = schema_for_output::<PromptDetailOut>()
    )]
    async fn create_prompt(
        &self,
        Parameters(args): Parameters<CreatePromptArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .blocking_db(move |db| {
                run_write(db, |db| {
                    let request = CreatePromptRequest {
                        title: args.title,
                        description: args.description,
                        content: args.content,
                        variant_label: args.variant_label,
                        tags: args.tags.unwrap_or_default(),
                        is_favorite: false,
                    };
                    run_transaction_fault(|driver, policy| {
                        prompt_service::create_prompt_with_driver(db, request, driver, policy)
                    })
                    .map(PromptDetailOut::from)
                })
            })
            .await;
        structured_result(result)
    }

    /// Get a playbook (ordered prompt workflow) with fully hydrated steps, by id or exact title.
    #[tool(
        annotations(
            read_only_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        ),
        output_schema = schema_for_output::<PlaybookDetailOut>()
    )]
    async fn get_playbook(
        &self,
        Parameters(args): Parameters<GetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .blocking_db(move |db| {
                maybe_panic();
                super::resolve_playbook(db, &args.id_or_title).and_then(PlaybookDetailOut::try_from)
            })
            .await;
        structured_result(result)
    }

    /// Get a prompt with all its variants, by id or exact title.
    #[tool(
        annotations(
            read_only_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        ),
        output_schema = schema_for_output::<PromptDetailOut>()
    )]
    async fn get_prompt(
        &self,
        Parameters(args): Parameters<GetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .blocking_db(move |db| {
                maybe_panic();
                super::resolve_prompt_detail(db, &args.id_or_title).map(PromptDetailOut::from)
            })
            .await;
        structured_result(result)
    }

    /// List playbooks with step counts.
    #[tool(
        annotations(
            read_only_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        ),
        output_schema = schema_for_output::<PlaybooksOut>()
    )]
    async fn list_playbooks(&self) -> Result<CallToolResult, ErrorData> {
        let result = self
            .blocking_db(|db| {
                playbook_service::list_playbooks_with_counts(&db.conn)?
                    .into_iter()
                    .map(PlaybookSummary::try_from)
                    .collect::<AppResult<Vec<_>>>()
                    .map(|playbooks| PlaybooksOut { playbooks })
            })
            .await;
        structured_result(result)
    }

    /// List prompts (filter: all, favorites, or recent).
    #[tool(
        annotations(
            read_only_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        ),
        output_schema = schema_for_output::<PageOut>()
    )]
    async fn list_prompts(
        &self,
        Parameters(args): Parameters<ListArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .blocking_db(move |db| {
                let (limit, offset, next_offset) = page(args.limit, args.offset)?;
                let filter = match args.filter.as_deref().unwrap_or("all") {
                    "all" => PromptFilter::All,
                    "favorites" => PromptFilter::Favorites,
                    "recent" => PromptFilter::Recent,
                    _ => {
                        return Err(AppError::invalid(
                            "filter must be one of: all, favorites, recent",
                        ));
                    }
                };
                let mut prompts =
                    prompt_service::list_prompt_summaries(&db.conn, filter, limit + 1, offset)?;
                let has_more = prompts.len() > limit as usize;
                prompts.truncate(limit as usize);
                Ok(PageOut {
                    prompts: prompts.into_iter().map(PromptSummary::from).collect(),
                    next_offset: has_more.then_some(next_offset),
                })
            })
            .await;
        structured_result(result)
    }

    /// List tags with prompt counts.
    #[tool(
        annotations(
            read_only_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        ),
        output_schema = schema_for_output::<TagsOut>()
    )]
    async fn list_tags(&self) -> Result<CallToolResult, ErrorData> {
        let result = self
            .blocking_db(|db| {
                tag_service::list_tags_with_counts(&db.conn)?
                    .into_iter()
                    .map(TagOut::try_from)
                    .collect::<AppResult<Vec<_>>>()
                    .map(|tags| TagsOut { tags })
            })
            .await;
        structured_result(result)
    }

    /// Record usage of a prompt and return its content (primary variant unless variant_id given).
    #[tool(
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        ),
        output_schema = schema_for_output::<RecordCopyOut>()
    )]
    async fn record_copy(
        &self,
        Parameters(args): Parameters<RecordCopyArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .blocking_db(move |db| {
                run_write(db, |db| {
                    run_transaction_fault(|driver, policy| {
                        prompt_service::record_copy_with_driver(
                            db,
                            &args.prompt_id,
                            args.variant_id.as_deref(),
                            driver,
                            policy,
                        )
                    })
                    .map(|content| RecordCopyOut { content })
                })
            })
            .await;
        structured_result(result)
    }

    /// Full-text search over prompts.
    #[tool(
        annotations(
            read_only_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        ),
        output_schema = schema_for_output::<PageOut>()
    )]
    async fn search_prompts(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .blocking_db(move |db| {
                let (limit, offset, next_offset) = page(args.limit, args.offset)?;
                let mut prompts = prompt_service::search_prompt_summaries(
                    &db.conn,
                    &args.query,
                    limit + 1,
                    offset,
                )?;
                let has_more = prompts.len() > limit as usize;
                prompts.truncate(limit as usize);
                Ok(PageOut {
                    prompts: prompts.into_iter().map(PromptSummary::from).collect(),
                    next_offset: has_more.then_some(next_offset),
                })
            })
            .await;
        structured_result(result)
    }

    /// Replace the content of a prompt's variant (primary unless variant_id given).
    #[tool(
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        ),
        output_schema = schema_for_output::<PromptDetailOut>()
    )]
    async fn update_prompt_content(
        &self,
        Parameters(args): Parameters<UpdateContentArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .blocking_db(move |db| {
                run_write(db, |db| {
                    run_transaction_fault(|driver, policy| {
                        prompt_service::update_prompt_content_with_driver(
                            db,
                            &args.prompt_id,
                            args.variant_id.as_deref(),
                            &args.content,
                            driver,
                            policy,
                        )
                    })
                    .map(PromptDetailOut::from)
                })
            })
            .await;
        structured_result(result)
    }
}

fn structured_result<T: serde::Serialize>(
    result: AppResult<T>,
) -> Result<CallToolResult, ErrorData> {
    match result {
        Ok(output) => ok_structured(&output),
        Err(error) => app_err(error),
    }
}

fn page(limit: Option<u32>, offset: Option<u32>) -> AppResult<(u32, u32, u32)> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let next_offset = offset
        .checked_add(limit)
        .ok_or_else(|| AppError::invalid("offset plus limit is too large"))?;
    Ok((limit, offset, next_offset))
}

fn run_write<T>(db: &mut Db, operation: impl FnOnce(&mut Db) -> AppResult<T>) -> AppResult<T> {
    let mut scope = WriteScope::open(db).map_err(AppError::from)?;
    let result = operation(scope.db());
    match scope.close() {
        Ok(()) => result,
        Err(_) => Err(AppError::internal("write scope close failed")),
    }
}

fn maybe_panic() {
    assert!(!fault::panic_in_tool(), "MCP fault: panic_in_tool");
}

#[cfg(feature = "test-faults")]
fn run_transaction_fault<T>(
    operation: impl FnOnce(
        &dyn cadence_core::services::transaction::TxDriver,
        &cadence_core::services::transaction::RetryPolicy,
    ) -> AppResult<T>,
) -> AppResult<T> {
    fault::with_tx_driver(operation)
}

#[cfg(not(feature = "test-faults"))]
fn run_transaction_fault<T>(
    operation: impl FnOnce(
        &dyn cadence_core::services::transaction::TxDriver,
        &cadence_core::services::transaction::RetryPolicy,
    ) -> AppResult<T>,
) -> AppResult<T> {
    operation(
        &cadence_core::services::transaction::RealDriver,
        &cadence_core::services::transaction::RetryPolicy::production(),
    )
}
