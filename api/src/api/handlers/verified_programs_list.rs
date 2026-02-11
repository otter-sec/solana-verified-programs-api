use crate::db::{
    models::{PaginationMeta, VerifiedProgramListResponse, VerifiedProgramsQuery},
    programs::PER_PAGE,
    DbClient,
};
use crate::validation;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use tracing::{error, info};

/// Handler for retrieving a list of all verified programs
///
/// # Endpoint: GET /verified-programs
///
/// # Returns
/// * `(StatusCode, Json<VerifiedProgramListResponse>)` - Status code and list of verified program addresses
///
/// On success, returns OK status with the list of program IDs
/// On failure, still returns an empty list but logs the error
pub(crate) async fn get_verified_programs_list(
    State(db): State<DbClient>,
    Query(query): Query<VerifiedProgramsQuery>,
) -> (StatusCode, Json<VerifiedProgramListResponse>) {
    info!("Fetching list of verified programs");
    get_verified_programs_list_paginated(State(db), Path(1), Query(query)).await
}

/// Handler for retrieving a paginated list of verified programs
///
/// # Endpoint: GET /verified-programs/:page
///
/// # Returns
/// * `(StatusCode, Json<VerifiedProgramListResponse>)` - Status code and list of verified program addresses
pub(crate) async fn get_verified_programs_list_paginated(
    State(db): State<DbClient>,
    Path(page): Path<i64>,
    Query(query): Query<VerifiedProgramsQuery>,
) -> (StatusCode, Json<VerifiedProgramListResponse>) {
    let page = page.max(1);

    let search: Option<&str> = query.search.as_deref();

    if let Some(s) = search {
        if let Some(msg) = validation::validate_search(s) {
            return (
                StatusCode::BAD_REQUEST,
                Json(VerifiedProgramListResponse {
                    meta: PaginationMeta {
                        total: 0,
                        page,
                        total_pages: 0,
                        items_per_page: PER_PAGE,
                        has_next_page: false,
                        has_prev_page: false,
                    },
                    verified_programs: vec![],
                    error: Some(msg.to_string()),
                }),
            );
        }
    }

    let (verified_programs, total) = match db.get_verified_program_ids_page(page, search).await {
        Ok(result) => result,
        Err(err) => {
            error!("Failed to fetch verified programs: {}", err);
            return (
                StatusCode::OK,
                Json(VerifiedProgramListResponse {
                    meta: PaginationMeta {
                        total: 0,
                        page,
                        total_pages: 0,
                        items_per_page: PER_PAGE,
                        has_next_page: false,
                        has_prev_page: false,
                    },
                    verified_programs: vec![],
                    error: None,
                }),
            );
        }
    };

    let total_pages = (total + PER_PAGE - 1) / PER_PAGE;

    (
        StatusCode::OK,
        Json(VerifiedProgramListResponse {
            meta: PaginationMeta {
                total,
                page,
                total_pages,
                items_per_page: PER_PAGE,
                has_next_page: page < total_pages,
                has_prev_page: page > 1,
            },
            verified_programs,
            error: None,
        }),
    )
}
