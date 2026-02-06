use crate::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct OauthRedirectQueryParams {
    code: String,
}

pub async fn oauth_redirect(
    State(app_state): State<AppState>,
    Query(query_params): Query<OauthRedirectQueryParams>,
) -> impl IntoResponse {
    let code = query_params.code;

    let body = match app_state.click_up_service.post_oauth_token(code).await {
        Ok(body) => body,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
        }
    };

    (StatusCode::OK, Json(body)).into_response()
}
