use crate::services::clickup::{ClickUpTaskResponseBody, ClickUpTimeInStatusResponseBody};
use async_recursion::async_recursion;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use tokio::{join, task::JoinSet};

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

    async fn get_task_time_in_status(
        &self,
        token: &str,
        task_id: &str,
    ) -> Result<ClickUpTimeInStatusResponseBody, ClickUpServiceError> {
        let url = format!("{}/api/v2/task/{task_id}/time_in_status", self.base_url);
        let response = match self
            .http_client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::AUTHORIZATION, token)
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
            Err(e) => Err(ClickUpServiceError::ParseError(Box::new(e), Some(text))),
        }
    }

    #[async_recursion]
    pub async fn get_task(
        &self,
        token: &str,
        task_id: &str,
    ) -> Result<ClickUpTaskResponseBody, ClickUpServiceError> {
        let url = format!("{}/api/v2/task/{task_id}", self.base_url);
        let task_future = async {
            let response = match self
                .http_client
                .get(url)
                .header(reqwest::header::ACCEPT, "application/json")
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(reqwest::header::AUTHORIZATION, token)
                .query(&[("include_subtasks", "true")])
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

            let task = match serde_json::from_str::<ClickUpTaskResponseBody>(&text) {
                Ok(task) => task,
                Err(e) => return Err(ClickUpServiceError::ParseError(Box::new(e), Some(text))),
            };

            Ok(task)
        };

        let task_time_in_status_future = self.get_task_time_in_status(token, task_id);

        let (task, task_time_in_status) = join!(task_future, task_time_in_status_future);

        let mut task = task?;
        let task_time_in_status = task_time_in_status?;

        task.time_in_status = Some(task_time_in_status);

        if let ClickUpTaskResponseBody {
            sub_tasks: Some(sub_tasks),
            ..
        } = &mut task
        {
            let mut requests = JoinSet::new();
            for sub_task_record in &mut *sub_tasks {
                let sub_task_id = sub_task_record.id.clone();
                let app_clone = self.clone();
                let token_clone = token.to_string();
                requests.spawn(async move { app_clone.get_task(&token_clone, &sub_task_id).await });
            }

            while let Some(request) = requests.join_next().await {
                let fetched_sub_task = match request {
                    Ok(Ok(fetched_sub_task)) => fetched_sub_task,
                    // This could be improved to collect all errors then return instead of returing
                    // at the first one.
                    Ok(Err(e)) => return Err(e),
                    Err(_) => return Err(ClickUpServiceError::UnexpectedError),
                };

                let position_of_sub_task = match sub_tasks
                    .iter()
                    .position(|sub_task| fetched_sub_task.id == sub_task.id)
                {
                    Some(p) => p,
                    None => return Err(ClickUpServiceError::UnexpectedError),
                };
                let sub_task = match sub_tasks.get_mut(position_of_sub_task) {
                    Some(sub_task) => sub_task,
                    None => return Err(ClickUpServiceError::UnexpectedError),
                };
                sub_task.task = Some(fetched_sub_task);
            }
        };

        Ok(task)
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
        let response = match self
            .http_client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::AUTHORIZATION, token)
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
    pub color: String,
    #[serde(rename = "profilePicture")]
    pub profile_picture: Option<String>,
}
