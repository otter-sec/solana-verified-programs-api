use crate::db::DbClient;
use crate::models::VerifiedProgramListResponse;
use axum::{extract::State, http::StatusCode, Json};

pub(crate) async fn get_verified_programs_list(
    State(db): State<DbClient>,
) -> (StatusCode, Json<VerifiedProgramListResponse>) {
    let verified_programs = db.get_verified_programs().await.unwrap();

    // get all program ids from the verified_programs
    let programs_list = verified_programs
        .iter()
        .map(|program| program.program_id.clone())
        .collect::<Vec<String>>();

    let response_data = VerifiedProgramListResponse {
        verified_programs: programs_list,
    };

    (StatusCode::OK, Json(response_data))
}
