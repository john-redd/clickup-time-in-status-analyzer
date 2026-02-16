use crate::{
    AppState, constants::session::CLICK_UP_AUTH_TOKEN,
    domain::generate_points_vs_time_spent_analysis, services::clickup::ClickUpServiceError,
};
use axum::{
    Form,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use tower_sessions::Session;

#[derive(Deserialize)]
pub struct GetTaskResponseBody {
    task_id: String,
}

pub async fn task(
    session: Session,
    State(app_state): State<AppState>,
    Form(body): Form<GetTaskResponseBody>,
) -> impl IntoResponse {
    let token: String = match session.get(CLICK_UP_AUTH_TOKEN).await {
        Ok(Some(token)) => token,
        Err(_) | Ok(None) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error.").into_response();
        }
    };

    let task = match app_state
        .click_up_service
        .get_task(&token, &body.task_id)
        .await
    {
        Ok(task) => task,
        Err(e) => {
            return match e {
                ClickUpServiceError::TimeInStatusNotEnabled => (
                    StatusCode::OK,
                    Html("<p>Time in status is not enabled for the selected workspace.</p>"),
                )
                    .into_response(),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error.").into_response(),
            };
        }
    };

    let task_as_string = generate_points_vs_time_spent_analysis(&task.into());

    let html_response_body = format!(
        r#"
    <pre>
        {task_as_string}
    </pre>
        "#
    );

    (StatusCode::OK, Html(html_response_body)).into_response()
}
