use crate::services::clickup::{ClickUpTaskResponseBody, ClickUpTimeInStatusResponseBody};
use chrono::{DateTime, Datelike, Duration, Utc};
use reqwest::header::HeaderValue;
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

pub static IN_PROGRESS_ORDER_INDEX: i32 = 5;

#[derive(Clone)]
pub struct ClickUpService {
    base_url: String,
    http_client: reqwest::blocking::Client,
    std_headers: reqwest::header::HeaderMap,
}

impl ClickUpService {
    pub fn new(token: String) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.append(
            reqwest::header::ACCEPT,
            HeaderValue::from_str("application/json").unwrap(),
        );
        headers.append(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_str("application/json").unwrap(),
        );
        headers.append(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(token.as_str()).unwrap(),
        );
        Self {
            base_url: "https://api.clickup.com".to_string(),
            http_client: reqwest::blocking::Client::new(),
            std_headers: headers,
        }
    }

    fn get_task_time_in_status(&self, task_id: &str) -> ClickUpTimeInStatusResponseBody {
        let url = format!("{}/api/v2/task/{task_id}/time_in_status", self.base_url);
        let response = self
            .http_client
            .get(url)
            .headers(self.std_headers.clone())
            .send();

        response
            .unwrap()
            .json::<ClickUpTimeInStatusResponseBody>()
            .unwrap()
    }

    pub fn get_task(&self, task_id: &str) -> ClickUpTaskResponseBody {
        let url = format!("{}/api/v2/task/{task_id}", self.base_url);
        let response = self
            .http_client
            .get(url)
            .headers(self.std_headers.clone())
            .query(&[("include_subtasks", "true")])
            .send();

        let mut task = response.unwrap().json::<ClickUpTaskResponseBody>().unwrap();
        let task_time_in_status = self.get_task_time_in_status(task_id);
        task.time_in_status = Some(task_time_in_status);

        let mut children = vec![];

        if let ClickUpTaskResponseBody {
            sub_tasks: Some(sub_tasks),
            ..
        } = &mut task
        {
            let (tx, rx): (
                Sender<ClickUpTaskResponseBody>,
                Receiver<ClickUpTaskResponseBody>,
            ) = mpsc::channel();
            for sub_task_record in &mut *sub_tasks {
                let thread_tx = tx.clone();
                let sub_task_id = sub_task_record.id.clone();
                let app_clone = self.clone();
                let child = thread::spawn(move || {
                    let sub_task = app_clone.get_task(&sub_task_id);
                    thread_tx.send(sub_task).unwrap();
                });
                children.push(child);
            }

            for _ in 0..sub_tasks.len() {
                let fetched_sub_task = rx.recv().unwrap();

                let position_of_sub_task = sub_tasks
                    .iter()
                    .position(|sub_task| fetched_sub_task.id == sub_task.id)
                    .unwrap();
                let sub_task = sub_tasks.get_mut(position_of_sub_task).unwrap();
                sub_task.task = Some(fetched_sub_task);
            }

            for child in children {
                child.join().expect("oops! the child thread panicked");
            }
        };

        task
    }
}

pub fn generate_points_vs_time_spent_analysis(task: &ClickUpTaskResponseBody) -> String {
    fn generate_points_vs_time_spent_analysis_iter(
        task: &ClickUpTaskResponseBody,
        mut prefix: String,
    ) -> String {
        let points = get_sprint_points(task);
        let time_in_status_count = get_days_in_dev_status(task);

        let mut result = format!(
            "\n{prefix}{} - points: {points}, time_spent: {time_in_status_count}",
            task.custom_id
        );

        prefix.push('\t');

        if let Some(sub_tasks) = &task.sub_tasks {
            for sub_task in sub_tasks {
                if let Some(next_task) = &sub_task.task {
                    let nested_result =
                        generate_points_vs_time_spent_analysis_iter(next_task, prefix.clone());
                    result.push_str(&nested_result);
                }
            }
        }

        result
    }

    generate_points_vs_time_spent_analysis_iter(task, "".to_string())
}

fn get_sprint_points(task: &ClickUpTaskResponseBody) -> f32 {
    task.points.unwrap_or_default()
}

fn get_days_in_dev_status(task: &ClickUpTaskResponseBody) -> i64 {
    let mut initial_dev_status_start: Option<DateTime<Utc>> = None;

    let total = task
        .time_in_status
        .as_ref()
        .unwrap()
        .status_history
        .iter()
        .filter_map(|s| match &s.order_index {
            Some(order_index) => {
                if *order_index >= IN_PROGRESS_ORDER_INDEX {
                    initial_dev_status_start = Some(s.total_time.since);
                    Some(s.total_time.by_minute / 60 / 24)
                } else {
                    None
                }
            }
            None => None,
        })
        .reduce(|acc, n| acc + n)
        .unwrap_or_default();

    match initial_dev_status_start {
        Some(mut cursor) => {
            let now = cursor + Duration::days(total);
            let mut days_to_ignore = 0;

            while cursor < now {
                match cursor.weekday() {
                    chrono::Weekday::Sat | chrono::Weekday::Sun => days_to_ignore += 1,
                    _ => {}
                }
                cursor += Duration::days(1);
            }

            total - days_to_ignore
        }
        None => total,
    }
}
