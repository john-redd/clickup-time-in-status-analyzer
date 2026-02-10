use crate::services::clickup::{ClickUpTaskResponseBody, ClickUpTimeInStatusResponseBody};
use async_recursion::async_recursion;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use tokio::join;

pub static IN_PROGRESS_ORDER_INDEX: i32 = 5;

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
        task_id: &str,
    ) -> Result<ClickUpTaskResponseBody, ClickUpServiceError> {
        get_task_tree(&self.http_client, &self.base_url, token, task_id).await
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
        dbg!(&request);
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
    task_id: &str,
) -> Result<ClickUpTaskResponseBody, ClickUpServiceError> {
    let mut task = get_task(http_client, base_url, token, task_id).await?;

    if let ClickUpTaskResponseBody {
        sub_tasks: Some(sub_tasks),
        ..
    } = &mut task
    {
        for sub_task_record in &mut *sub_tasks {
            let task = get_task_tree(http_client, base_url, token, &sub_task_record.id).await?;
            sub_task_record.task = Some(task)
        }
    };

    Ok(task)
}

pub async fn get_task(
    http_client: &reqwest::Client,
    base_url: &str,
    token: &str,
    task_id: &str,
) -> Result<ClickUpTaskResponseBody, ClickUpServiceError> {
    let url = format!("{base_url}/api/v2/task/{task_id}");
    println!("token: {token}");
    let task_future = async {
        let request = http_client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
            .query(&[("include_subtasks", "true")]);

        dbg!(&request);
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
                return Err(ClickUpServiceError::ParseError(
                    Box::new(e),
                    Some(format!("get_task = {status_code} {text}")),
                ));
            }
        };

        Ok(task)
    };

    let task_time_in_status_future = get_task_time_in_status(http_client, base_url, token, task_id);

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
    task_id: &str,
) -> Result<ClickUpTimeInStatusResponseBody, ClickUpServiceError> {
    let url = format!("{base_url}/api/v2/task/{task_id}/time_in_status");
    let response = match http_client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
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
        Err(e) => return Err(ClickUpServiceError::ParseError(Box::new(e), None)),
    };

    match serde_json::from_str::<ClickUpTimeInStatusResponseBody>(&text) {
        Ok(v) => Ok(v),
        Err(e) => Err(ClickUpServiceError::ParseError(
            Box::new(e),
            Some(format!("get_task_time_in_status {text}")),
        )),
    }
}
