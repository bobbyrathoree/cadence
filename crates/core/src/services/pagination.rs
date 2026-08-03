use crate::error::{AppError, AppResult};

pub const DEFAULT_PAGE_SIZE: i64 = 100;
pub const MAX_PAGE_SIZE: i64 = 500;
pub const MAX_SEARCH_RESULTS: i64 = 100;

pub fn sanitize_pagination(limit: Option<i64>, offset: Option<i64>) -> AppResult<(i64, i64)> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);
    let offset = offset.unwrap_or(0);
    if limit < 1 {
        return Err(AppError::invalid("limit must be at least 1"));
    }
    if offset < 0 {
        return Err(AppError::invalid("offset must be at least 0"));
    }
    Ok((limit.min(MAX_PAGE_SIZE), offset))
}

pub fn sanitize_search_limit(limit: Option<i64>) -> AppResult<i64> {
    let limit = limit.unwrap_or(MAX_SEARCH_RESULTS);
    if limit < 1 {
        return Err(AppError::invalid("limit must be at least 1"));
    }
    Ok(limit.min(MAX_SEARCH_RESULTS))
}
