use crate::services::clickup::{ClickUpTaskResponseBody, ClickUpTimeInStatusResponseBody};
use async_recursion::async_recursion;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use tokio::{join, task::JoinSet, time::Instant};

pub static IN_PROGRESS_ORDER_INDEX: i32 = 5;

const TIME_IN_STATUS_NOT_ENABLED_ERROR_CODE: &str = "TIS_027";
const NOT_AUTHORIZED_ERROR_CODE: &str = "OAUTH_018";

#[derive(Clone)]
pub struct ClickUpService {
    base_url: String,
    http_client: reqwest::Client,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

#[derive(Debug)]
pub enum ClickUpServiceError {
    FailedToSendNetworkRequestError(Box<dyn Error + Send + 'static>),
    ParseError(Box<dyn Error + Send + 'static>, Option<String>),
    UnexpectedError,
    TimeInStatusNotEnabled,
    CustomIDError,
}

#[derive(Clone)]
pub struct GetTaskRequest {
    pub task_id: String,
    pub workspace_id: Option<String>,
}

impl ClickUpService {
    pub fn new(client_id: &str, client_secret: &str, redirect_uri: &str) -> Self {
        Self {
            base_url: "https://api.clickup.com".to_string(),
            http_client: reqwest::Client::new(),
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            redirect_uri: redirect_uri.to_string(),
        }
    }

    pub async fn get_task(
        &self,
        token: &str,
        mut request_config: GetTaskRequest,
    ) -> Result<ClickUpTaskResponseBody, ClickUpServiceError> {
        get_task_tree(
            &self.http_client,
            &self.base_url,
            token,
            &mut request_config,
        )
        .await
    }

    pub fn generate_oauth_login_redirect_url(&self) -> Result<url::Url, ClickUpServiceError> {
        match url::Url::parse_with_params(
            "https://app.clickup.com/api",
            &[
                ("client_id", self.client_id.clone()),
                ("redirect_uri", self.redirect_uri.clone()),
            ],
        ) {
            Ok(url) => Ok(url),
            Err(_) => Err(ClickUpServiceError::UnexpectedError),
        }
    }

    pub async fn post_oauth_token(
        &self,
        code: String,
    ) -> Result<ClickUpOauthTokenResponseBody, ClickUpServiceError> {
        let url = format!("{}/api/v2/oauth/token", self.base_url);
        let response = match self
            .http_client
            .post(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&json!({
                "client_id": self.client_id,
                "client_secret": self.client_secret,
                "code": code,
            }))
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                return Err(ClickUpServiceError::FailedToSendNetworkRequestError(
                    Box::new(e),
                ));
            }
        };

        let text = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                return Err(ClickUpServiceError::ParseError(Box::new(e), None));
            }
        };

        let body = match serde_json::from_str::<ClickUpOauthTokenResponseBody>(&text) {
            Ok(body) => body,
            Err(e) => return Err(ClickUpServiceError::ParseError(Box::new(e), Some(text))),
        };

        Ok(body)
    }

    pub async fn get_authorized_workspaces(
        &self,
        token: String,
    ) -> Result<ClickUpGetWorkspacesResponseBody, ClickUpServiceError> {
        let url = format!("{}/api/v2/team", self.base_url);
        let request = self
            .http_client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));
        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                return Err(ClickUpServiceError::FailedToSendNetworkRequestError(
                    Box::new(e),
                ));
            }
        };

        let text = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                return Err(ClickUpServiceError::ParseError(Box::new(e), None));
            }
        };

        let body = match serde_json::from_str::<ClickUpGetWorkspacesResponseBody>(&text) {
            Ok(body) => body,
            Err(e) => {
                return Err(ClickUpServiceError::ParseError(Box::new(e), Some(text)));
            }
        };

        Ok(body)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ClickUpOauthTokenResponseBody {
    pub access_token: String,
}

#[derive(Serialize, Deserialize)]
pub struct ClickUpGetWorkspacesResponseBody {
    pub teams: Vec<ClickUpWorkspace>,
}

#[derive(Serialize, Deserialize)]
pub struct ClickUpWorkspace {
    pub id: String,
    pub name: String,
    pub color: String,
    pub avatar: Option<String>,
    pub members: Vec<ClickUpWorkspaceMember>,
}

#[derive(Serialize, Deserialize)]
pub struct ClickUpWorkspaceMember {
    pub user: ClickUpWorkspaceUser,
}

#[derive(Serialize, Deserialize)]
pub struct ClickUpWorkspaceUser {
    pub id: i32,
    pub username: String,
    pub color: Option<String>,
    #[serde(rename = "profilePicture")]
    pub profile_picture: Option<String>,
}

#[async_recursion]
async fn get_task_tree(
    http_client: &reqwest::Client,
    base_url: &str,
    token: &str,
    request_config: &mut GetTaskRequest,
) -> Result<ClickUpTaskResponseBody, ClickUpServiceError> {
    let mut task = get_task(http_client, base_url, token, &request_config.clone()).await?;

    if let ClickUpTaskResponseBody {
        sub_tasks: Some(sub_tasks),
        ..
    } = &mut task
    {
        let mut requests = futures::stream::FuturesUnordered::new();
        for (i, sub_task_record) in sub_tasks.iter().enumerate() {
            let sub_task_id = sub_task_record.id.clone();
            request_config.task_id = sub_task_id;
            let mut request_config_clone = request_config.clone();
            requests.push(async move {
                let task =
                    get_task_tree(http_client, base_url, token, &mut request_config_clone).await;

                (i, task)
            });
        }

        while let Some((i, sub_task_request)) = requests.next().await {
            let task = match sub_task_request {
                Ok(task) => task,
                Err(e) => return Err(e),
            };

            if let Some(sub_task) = sub_tasks.get_mut(i) {
                sub_task.task = Some(task);
            };
        }
    };

    Ok(task)
}

pub async fn get_task(
    http_client: &reqwest::Client,
    base_url: &str,
    token: &str,
    request_config: &GetTaskRequest,
) -> Result<ClickUpTaskResponseBody, ClickUpServiceError> {
    let url = format!("{base_url}/api/v2/task/{}", request_config.task_id);
    let task_future = async {
        let mut query_params = vec![("include_subtasks", "true")];

        if let Some(workspace_id) = &request_config.workspace_id {
            query_params.push(("custom_task_ids", "true"));
            query_params.push(("team_id", workspace_id.as_str()));
        }

        let request = http_client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
            .query(&query_params);

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                return Err(ClickUpServiceError::FailedToSendNetworkRequestError(
                    Box::new(e),
                ));
            }
        };

        let status_code = response.status();

        let text = match response.text().await {
            Ok(text) => text,
            Err(e) => return Err(ClickUpServiceError::ParseError(Box::new(e), None)),
        };

        let task = match serde_json::from_str::<ClickUpTaskResponseBody>(&text) {
            Ok(task) => task,
            Err(e) => {
                if text.contains(NOT_AUTHORIZED_ERROR_CODE) && request_config.workspace_id.is_none()
                {
                    return Err(ClickUpServiceError::CustomIDError);
                } else {
                    return Err(ClickUpServiceError::ParseError(
                        Box::new(e),
                        Some(format!("get_task = {status_code} {text}")),
                    ));
                }
            }
        };

        Ok(task)
    };

    let task_time_in_status_future =
        get_task_time_in_status(http_client, base_url, token, request_config);

    let (task, task_time_in_status) = join!(task_future, task_time_in_status_future);

    let mut task = task?;
    let task_time_in_status = task_time_in_status?;

    task.time_in_status = Some(task_time_in_status);

    Ok(task)
}

async fn get_task_time_in_status(
    http_client: &reqwest::Client,
    base_url: &str,
    token: &str,
    request_config: &GetTaskRequest,
) -> Result<ClickUpTimeInStatusResponseBody, ClickUpServiceError> {
    let url = format!(
        "{base_url}/api/v2/task/{}/time_in_status",
        request_config.task_id
    );
    let mut request = http_client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));

    if let Some(workspace_id) = &request_config.workspace_id {
        request = request.query(&[
            ("custom_task_ids", "true"),
            ("team_id", workspace_id.as_str()),
        ]);
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(e) => {
            return Err(ClickUpServiceError::FailedToSendNetworkRequestError(
                Box::new(e),
            ));
        }
    };

    let text = match response.text().await {
        Ok(text) => text,
        Err(e) => return Err(ClickUpServiceError::ParseError(Box::new(e), None)),
    };

    match serde_json::from_str::<ClickUpTimeInStatusResponseBody>(&text) {
        Ok(v) => Ok(v),
        Err(e) => {
            if text.contains(TIME_IN_STATUS_NOT_ENABLED_ERROR_CODE) {
                Err(ClickUpServiceError::TimeInStatusNotEnabled)
            } else if text.contains(NOT_AUTHORIZED_ERROR_CODE)
                && request_config.workspace_id.is_none()
            {
                Err(ClickUpServiceError::CustomIDError)
            } else {
                Err(ClickUpServiceError::ParseError(
                    Box::new(e),
                    Some(format!("get_task_time_in_status {text}")),
                ))
            }
        }
    }
}
