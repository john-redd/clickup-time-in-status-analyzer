use axum::{Json, response::IntoResponse};
use tower_sessions::Session;
use reqwest::StatusCode;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct PutWorkspaceBody {
    workspace_id: String
}

pub async fn put_workspace(session: Session, Json(body): Json<PutWorkspaceBody>) -> impl IntoResponse {
    match session
        .insert(
            crate::constants::session::CURRENT_WORKSPACE_ID,
            body.workspace_id,
        )
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}
