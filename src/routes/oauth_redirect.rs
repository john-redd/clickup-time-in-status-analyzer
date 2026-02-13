use crate::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use serde::Deserialize;
use tower_sessions::Session;

#[derive(Deserialize)]
pub struct OauthRedirectQueryParams {
    code: String,
}

pub async fn oauth_redirect(
    State(app_state): State<AppState>,
    Query(query_params): Query<OauthRedirectQueryParams>,
    session: Session,
) -> impl IntoResponse {
    let code = query_params.code;

    let body = match app_state.click_up_service.post_oauth_token(code).await {
        Ok(body) => body,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
        }
    };

    match session
        .insert(
            crate::constants::session::CLICK_UP_AUTH_TOKEN,
            body.access_token,
        )
        .await
    {
        Ok(_) => Redirect::temporary("/home").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}
