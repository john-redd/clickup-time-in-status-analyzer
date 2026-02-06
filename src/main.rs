use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderName};
use axum::response::Redirect;
use axum::routing::post;
use axum::{Router, routing::get, serve};
use axum::{http::StatusCode, response::IntoResponse};
use clickup_time_in_status_analyzer::{
    domain::{Ticket, generate_points_vs_time_spent_analysis},
    services::clickup::ClickUpService,
};
use serde::{Deserialize, Serialize, de};
use serde_json::json;
use std::error::Error;
use std::sync::Arc;
use axum::{Json, debug_handler};

// static TASK: &str = "86aea18zr";
static TASK: &str = "86a8jcehg";
// static TASK: &str = "86aebe0xh";
// static TASK: &str = "86aefze6c";

async fn get_health() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

async fn login(State(app_state): State<AppState>) -> impl IntoResponse {
    let url = match url::Url::parse_with_params(
        "https://app.clickup.com/api",
        &[
            ("client_id", app_state.click_up_client_id),
            ("redirect_uri", app_state.click_up_redirect_uri),
        ],
    ) {
        Ok(url) => url,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
        }
    };

    Redirect::permanent(url.as_str()).into_response()
}

#[derive(Deserialize)]
struct OauthRedirectQueryParams {
    code: String,
}

#[derive(Serialize, Deserialize)]
struct ClickUpOauthTokenResponseBody {
    access_token: String,
}

#[debug_handler]
async fn oauth_redirect(
    State(app_state): State<AppState>,
    Query(query_params): Query<OauthRedirectQueryParams>,
) -> impl IntoResponse {
    let code = query_params.code;

    let client = reqwest::Client::new();

    let response = match client
        .post("https://api.clickup.com/api/v2/oauth/token")
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&json!({
            "client_id": app_state.click_up_client_id,
            "client_secret": app_state.click_up_client_secret,
            "code": code,
        }))
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
        }
    };

    if response.status() != StatusCode::OK {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
    }

    let body = match response.json::<ClickUpOauthTokenResponseBody>().await {
        Ok(body) => body,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
        }
    };

    (StatusCode::OK, Json(body)).into_response()
}

// https://app.clickup.com/api?client_id={client_id}&redirect_uri={redirect_uri}

#[derive(Clone)]
struct AppState {
    click_up_client_id: String,
    click_up_client_secret: String,
    click_up_redirect_uri: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let click_up_client_id =
        std::env::var("CLICK_UP_CLIENT_ID").expect("failed to find CLICK_UP_CLIENT_ID env var.");
    let click_up_client_secret = std::env::var("CLICK_UP_CLIENT_SECRET")
        .expect("failed to find CLICK_UP_CLIENT_SECRET env var.");
    let click_up_redirect_uri = std::env::var("CLICK_UP_REDIRECT_URI")
        .expect("failed to find CLICK_UP_REDIRECT_URI env var.");
    let app_state = AppState {
        click_up_client_id,
        click_up_client_secret,
        click_up_redirect_uri,
    };

    let listener = tokio::net::TcpListener::bind("0.0.0.0:13000").await?;

    let app = Router::new()
        .route("/api/v1/health", get(get_health))
        .route("/login", get(login))
        .route("/oauth/redirect", get(oauth_redirect))
        .with_state(app_state);

    let server = serve(listener, app);

    server.await?;

    Ok(())
}
