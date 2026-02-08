use crate::{AppState, components::Workspace, constants::session::{CLICK_UP_AUTH_TOKEN, CURRENT_WORKSPACE_ID}};
use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use tower_sessions::Session;

#[derive(Template)]
#[template(path = "home.html")]
struct HomePage {
    current_workspace_id: String,
    workspaces: Vec<Workspace>,
}

pub async fn home(session: Session, State(app_state): State<AppState>) -> impl IntoResponse {
    let token: String = session.get(CLICK_UP_AUTH_TOKEN).await.unwrap().unwrap();
    let current_workspace_id: String = match session.get(CURRENT_WORKSPACE_ID).await.unwrap()
    {
        Some(v) => v,
        None => "".to_string(),
    };
    let workspaces: Vec<Workspace> = match app_state
        .click_up_service
        .get_authorized_workspaces(token)
        .await
    {
        Ok(workspaces) => workspaces
            .teams
            .iter()
            .map(|t| Workspace {
                id: t.id.clone(),
                name: t.name.clone(),
            })
            .collect(),
        Err(e) => {
            println!("{:?}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
        }
    };

    let home_page = HomePage {
        current_workspace_id,
        workspaces,
    };

    let html_response_body = match home_page.render() {
        Ok(html_response_body) => html_response_body,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
        }
    };

    Html(html_response_body).into_response()
}
