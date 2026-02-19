use crate::{
    AppState,
    constants::session::{CLICK_UP_AUTH_TOKEN, CURRENT_WORKSPACE_ID},
    domain::{Task, apply_no_weekends_formula, generate_points_vs_time_spent_analysis},
    services::clickup::{ClickUpServiceError, GetTaskRequest},
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
#[serde(from = "Option<String>")]
struct HtmlCheckbox(bool);

impl From<Option<String>> for HtmlCheckbox {
    fn from(value: Option<String>) -> Self {
        match value {
            Some(v) => match v.as_str() {
                "on" => HtmlCheckbox(true),
                _ => HtmlCheckbox(false),
            },
            None => HtmlCheckbox(false),
        }
    }
}

#[derive(Deserialize)]
pub struct PostTaskResponseBody {
    task_id: String,
    remove_weekends: HtmlCheckbox,
    use_custom_id: HtmlCheckbox,
}

pub async fn task(
    session: Session,
    State(app_state): State<AppState>,
    Form(body): Form<PostTaskResponseBody>,
) -> impl IntoResponse {
    if body.task_id.is_empty() {
        return (StatusCode::OK, Html("<p>Missing task id.</p>")).into_response();
    }
    let token: String = match session.get(CLICK_UP_AUTH_TOKEN).await {
        Ok(Some(token)) => token,
        Err(_) | Ok(None) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error.").into_response();
        }
    };

    let mut workspace_id = match session.get(CURRENT_WORKSPACE_ID).await {
        Ok(Some(workspace_id)) => workspace_id,
        Err(_) | Ok(None) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error.").into_response();
        }
    };

    if !body.use_custom_id.0 {
        workspace_id = None
    }

    let task = match app_state
        .click_up_service
        .get_task(
            &token,
            GetTaskRequest {
                task_id: body.task_id,
                workspace_id,
            },
        )
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
                ClickUpServiceError::CustomIDError => (
                    StatusCode::OK,
                    Html("<p>You might be using a custom id without setting the `Use Custom ID` field to true.</p>"),
                )
                    .into_response(),
                e => {
                    println!("{e:?}");
                    return (StatusCode::OK, Html("<p>Something went wrong, please review the information in the form and try again</p>")).into_response()},
            };
        }
    };

    let mut task = Task::from(task);
    if body.remove_weekends.0 {
        apply_no_weekends_formula(&mut task);
    }
    let task_as_string = generate_points_vs_time_spent_analysis(&task);

    let html_response_body = format!(
        r#"
    <pre>
        {task_as_string}
    </pre>
        "#
    );

    (StatusCode::OK, Html(html_response_body)).into_response()
}
