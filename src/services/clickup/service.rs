use crate::services::clickup::{ClickUpTaskResponseBody, ClickUpTimeInStatusResponseBody};
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::task::JoinSet;
use async_recursion::async_recursion;

pub static IN_PROGRESS_ORDER_INDEX: i32 = 5;

#[derive(Clone)]
pub struct ClickUpService {
    base_url: String,
    http_client: reqwest::Client,
    std_headers: reqwest::header::HeaderMap,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

pub enum ClickUpServiceError {
    UnexpectedError,
}

impl ClickUpService {
    pub fn new(client_id: &str, client_secret: &str, redirect_uri: &str) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.append(
            reqwest::header::ACCEPT,
            HeaderValue::from_str("application/json").unwrap(),
        );
        headers.append(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_str("application/json").unwrap(),
        );
        Self {
            base_url: "https://api.clickup.com".to_string(),
            http_client: reqwest::Client::new(),
            std_headers: headers,
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            redirect_uri: redirect_uri.to_string(),
        }
    }

    async fn get_task_time_in_status(&self, task_id: &str) -> ClickUpTimeInStatusResponseBody {
        let url = format!("{}/api/v2/task/{task_id}/time_in_status", self.base_url);
        let response = self
            .http_client
            .get(url)
            .headers(self.std_headers.clone())
            .send()
            .await;

        response
            .unwrap()
            .json::<ClickUpTimeInStatusResponseBody>()
            .await
            .unwrap()
    }

    #[async_recursion]
    pub async fn get_task(&self, task_id: &str) -> ClickUpTaskResponseBody {
        let url = format!("{}/api/v2/task/{task_id}", self.base_url);
        let response = self
            .http_client
            .get(url)
            .headers(self.std_headers.clone())
            .query(&[("include_subtasks", "true")])
            .send()
            .await;

        let mut task = response
            .unwrap()
            .json::<ClickUpTaskResponseBody>()
            .await
            .unwrap();
        let task_time_in_status = self.get_task_time_in_status(task_id).await;
        task.time_in_status = Some(task_time_in_status);

        if let ClickUpTaskResponseBody {
            sub_tasks: Some(sub_tasks),
            ..
        } = &mut task
        {
            for sub_task_record in &mut *sub_tasks {
                // let sub_task_id = sub_task_record.id.clone();
                // let app_clone = self.clone();
                // requests.spawn(app_clone.get_task(&sub_task_id));
                let task = self.get_task(&sub_task_record.id).await;
                sub_task_record.task = Some(task);
            }

            // while let Some(request) = requests.join_next().await {
            //     let fetched_sub_task = request.unwrap();
            //
            //     let position_of_sub_task = sub_tasks
            //         .iter()
            //         .position(|sub_task| fetched_sub_task.id == sub_task.id)
            //         .unwrap();
            //     let sub_task = sub_tasks.get_mut(position_of_sub_task).unwrap();
            //     sub_task.task = Some(fetched_sub_task);
            // }
        };

        task
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
            Err(_) => return Err(ClickUpServiceError::UnexpectedError),
        };

        let body = match response.json::<ClickUpOauthTokenResponseBody>().await {
            Ok(body) => body,
            Err(_) => return Err(ClickUpServiceError::UnexpectedError),
        };

        Ok(body)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ClickUpOauthTokenResponseBody {
    access_token: String,
}
