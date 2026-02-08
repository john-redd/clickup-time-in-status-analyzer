use axum::{Router, routing::get, serve};
use clickup_time_in_status_analyzer::AppState;
use clickup_time_in_status_analyzer::routes::{health, login, oauth_redirect};
use clickup_time_in_status_analyzer::services::clickup::ClickUpService;
use std::error::Error;
use std::sync::Arc;
use tower_sessions::{MemoryStore, SessionManagerLayer};
use clickup_time_in_status_analyzer::routes::session::put_workspace;
use axum::routing::put;
use clickup_time_in_status_analyzer::routes::pages::home;

// static TASK: &str = "86aea18zr";
// static TASK: &str = "86a8jcehg";
// static TASK: &str = "86aebe0xh";
// static TASK: &str = "86aefze6c";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let click_up_client_id =
        std::env::var("CLICK_UP_CLIENT_ID").expect("failed to find CLICK_UP_CLIENT_ID env var.");
    let click_up_client_secret = std::env::var("CLICK_UP_CLIENT_SECRET")
        .expect("failed to find CLICK_UP_CLIENT_SECRET env var.");
    let click_up_redirect_uri = std::env::var("CLICK_UP_REDIRECT_URI")
        .expect("failed to find CLICK_UP_REDIRECT_URI env var.");
    let click_up_service = ClickUpService::new(
        &click_up_client_id,
        &click_up_client_secret,
        &click_up_redirect_uri,
    );
    let app_state = AppState {
        click_up_service: Arc::new(click_up_service),
    };

    let listener = tokio::net::TcpListener::bind("0.0.0.0:13000").await?;

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_same_site(tower_sessions::cookie::SameSite::Lax);

    let app = Router::new()
        .route("/api/v1/health", get(health))
        .route("/login", get(login))
        .route("/oauth/redirect", get(oauth_redirect))
        .route("/home", get(home))
        .route("/session/workspace", put(put_workspace))
        .layer(session_layer)
        .with_state(app_state);

    let server = serve(listener, app);

    server.await?;

    Ok(())
}
