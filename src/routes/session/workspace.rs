use crate::{
    AppState,
    components::{Workspace, WorkspaceSelect},
    constants::session::CLICK_UP_AUTH_TOKEN,
};
use askama::Template;
use axum::{
    Form,
    extract::State,
    response::{Html, IntoResponse},
};
use reqwest::StatusCode;
use serde::Deserialize;
use tower_sessions::Session;

#[derive(Deserialize)]
pub struct PutWorkspaceBody {
    workspace_id: String,
}

pub async fn put_workspace(
    session: Session,
    State(app_state): State<AppState>,
    Form(body): Form<PutWorkspaceBody>,
) -> impl IntoResponse {
    if let Err(_) = session
        .insert(
            crate::constants::session::CURRENT_WORKSPACE_ID,
            &body.workspace_id,
        )
        .await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
    };

    let token: String = session.get(CLICK_UP_AUTH_TOKEN).await.unwrap().unwrap();

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
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
        }
    };

    let workspace_select = WorkspaceSelect {
        current_workspace_id: body.workspace_id,
        workspaces,
    };

    let html_response_body = match workspace_select.render() {
        Ok(html_response_body) => html_response_body,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
        }
    };

    Html(html_response_body).into_response()
}
