use crate::AppState;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use tower_sessions::Session;

pub async fn workspaces(session: Session, State(app_state): State<AppState>) -> impl IntoResponse {
    let token: String = session.get("click_up_access_token").await.unwrap().unwrap();
    let workspaces = match app_state
        .click_up_service
        .get_authorized_workspaces(token)
        .await
    {
        Ok(workspaces) => workspaces,
        Err(e) => {
            println!("{:?}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
        }
    };

    (StatusCode::OK, Json(workspaces)).into_response()
}
