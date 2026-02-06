use crate::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Redirect},
};

pub async fn login(State(app_state): State<AppState>) -> impl IntoResponse {
    let url = match app_state
        .click_up_service
        .generate_oauth_login_redirect_url()
    {
        Ok(url) => url,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
        }
    };

    Redirect::permanent(url.as_str()).into_response()
}
