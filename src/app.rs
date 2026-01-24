use crate::{ClickUpTaskResponseBody, ClickUpTimeInStatusResponseBody, SubTask};
use chrono::{DateTime, Datelike, Duration, Utc};
use reqwest::header::HeaderValue;
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

static IN_PROGRESS_ORDER_INDEX: i32 = 5;

#[derive(Clone)]
pub enum AggregrationMethod {
    Leaf,
    Node,
    NodeAndLeaf,
}

#[derive(Clone)]
pub struct Application {
    aggregation_method: AggregrationMethod,
    base_url: String,
    http_client: reqwest::blocking::Client,
    std_headers: reqwest::header::HeaderMap,
}

impl Application {
    pub fn new(aggregation_method: AggregrationMethod, token: String) -> Self {
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
            aggregation_method,
            base_url: "https://api.clickup.com".to_string(),
            http_client: reqwest::blocking::Client::new(),
            std_headers: headers,
        }
    }

    pub fn generate_points_vs_time_spent_analysis(&self, task: &ClickUpTaskResponseBody) -> String {
        fn generate_points_vs_time_spent_analysis_iter(
            app: &Application,
            task: &ClickUpTaskResponseBody,
            mut prefix: String,
        ) -> String {
            let points = app.get_sprint_points(task);
            let time_in_status_count = app.get_days_in_dev_status(task);

            let mut result = format!(
                "\n{prefix}{} - points: {points}, time_spent: {time_in_status_count}",
                task.custom_id
            );

            prefix.push('\t');

            if let Some(sub_tasks) = &task.sub_tasks {
                for sub_task in sub_tasks {
                    if let Some(next_task) = &sub_task.task {
                        let nested_result = generate_points_vs_time_spent_analysis_iter(
                            app,
                            next_task,
                            prefix.clone(),
                        );
                        result.push_str(&nested_result);
                    }
                }
            }

            result
        }

        generate_points_vs_time_spent_analysis_iter(self, task, "".to_string())
    }

    // pub fn get_task(&self, task_id: &str) -> ClickUpTaskResponseBody {
    //     let url = format!("{}/api/v2/task/{task_id}", self.base_url);
    //     let response = self
    //         .http_client
    //         .get(url)
    //         .headers(self.std_headers.clone())
    //         .query(&[("include_subtasks", "true")])
    //         .send();
    //
    //     let mut task = response.unwrap().json::<ClickUpTaskResponseBody>().unwrap();
    //     let task_time_in_status = self.get_task_time_in_status(task_id);
    //     task.time_in_status = Some(task_time_in_status);
    //
    //     let mut children = vec![];
    //
    //     if let (
    //         AggregrationMethod::Leaf | AggregrationMethod::NodeAndLeaf | AggregrationMethod::Node,
    //         ClickUpTaskResponseBody {
    //             sub_tasks: Some(sub_tasks),
    //             ..
    //         },
    //     ) = (&self.aggregation_method, &mut task)
    //     {
    //         let (tx, rx): (
    //             Sender<ClickUpTaskResponseBody>,
    //             Receiver<ClickUpTaskResponseBody>,
    //         ) = mpsc::channel();
    //         for sub_task_record in &mut *sub_tasks {
    //             let thread_tx = tx.clone();
    //             let sub_task_id = sub_task_record.id.clone();
    //             let child = thread::spawn(move || {
    //                 let sub_task = self.get_task(&sub_task_id);
    //                 thread_tx.send(sub_task).unwrap();
    //             });
    //             children.push(child);
    //             // sub_task_record.task = Some(sub_task);
    //         }
    //
    //         let mut sub_tasks_from_threads = Vec::with_capacity(sub_tasks.len());
    //         for _ in 0..sub_tasks.len() {
    //             // The `recv` method picks a message from the channel
    //             // `recv` will block the current thread if there are no messages available
    //             sub_tasks_from_threads.push(rx.recv());
    //         }
    //
    //         // Wait for the threads to complete any remaining work
    //         for child in children {
    //             child.join().expect("oops! the child thread panicked");
    //         }
    //     };
    //
    //     dbg!(&task);
    //
    //     task
    // }

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

    fn get_sprint_points(&self, task: &ClickUpTaskResponseBody) -> f32 {
        let task_points = task.points.unwrap_or_default();
        let leaf_points = {
            let mut sub_task_total = 0_f32;
            // TODO: Make recursive
            if let Some(sub_tasks) = &task.sub_tasks {
                sub_task_total = sub_tasks
                    .iter()
                    .filter_map(|s| s.points)
                    .reduce(|acc, points| acc + points)
                    .unwrap_or_default();
            }

            sub_task_total
        };

        match self.aggregation_method {
            AggregrationMethod::Leaf => leaf_points,
            AggregrationMethod::Node => task_points,
            AggregrationMethod::NodeAndLeaf => task_points + leaf_points,
        }
    }

    fn get_days_in_dev_status(&self, task: &ClickUpTaskResponseBody) -> i64 {
        let node_time = {
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
        };

        let leaf_time = { 0 };

        match self.aggregation_method {
            AggregrationMethod::Leaf => leaf_time,
            AggregrationMethod::Node => node_time,
            AggregrationMethod::NodeAndLeaf => node_time + leaf_time,
        }
    }
}

pub fn get_task(application: &Application, task_id: &str) -> ClickUpTaskResponseBody {
    let url = format!("{}/api/v2/task/{task_id}", application.base_url);
    let response = application
        .http_client
        .get(url)
        .headers(application.std_headers.clone())
        .query(&[("include_subtasks", "true")])
        .send();

    let mut task = response.unwrap().json::<ClickUpTaskResponseBody>().unwrap();
    let task_time_in_status = application.get_task_time_in_status(task_id);
    task.time_in_status = Some(task_time_in_status);

    let mut children = vec![];

    if let (
        AggregrationMethod::Leaf | AggregrationMethod::NodeAndLeaf | AggregrationMethod::Node,
        ClickUpTaskResponseBody {
            sub_tasks: Some(sub_tasks),
            ..
        },
    ) = (&application.aggregation_method, &mut task)
    {
        let (tx, rx): (
            Sender<ClickUpTaskResponseBody>,
            Receiver<ClickUpTaskResponseBody>,
        ) = mpsc::channel();
        for sub_task_record in &mut *sub_tasks {
            let thread_tx = tx.clone();
            let sub_task_id = sub_task_record.id.clone();
            let app_clone = application.clone();
            let child = thread::spawn(move || {
                let sub_task = get_task(&app_clone, &sub_task_id);
                thread_tx.send(sub_task).unwrap();
            });
            children.push(child);
        }

        // let mut sub_tasks_from_threads = Vec::with_capacity(sub_tasks.len());
        for _ in 0..sub_tasks.len() {
            // The `recv` method picks a message from the channel
            // `recv` will block the current thread if there are no messages available
            // sub_tasks_from_threads.push(rx.recv().unwrap());
            let fetched_sub_task = rx.recv().unwrap();

            let position_of_sub_task = sub_tasks
                .iter()
                .position(|sub_task| fetched_sub_task.id == sub_task.id)
                .unwrap();
            let sub_task = sub_tasks.get_mut(position_of_sub_task).unwrap();
            sub_task.task = Some(fetched_sub_task);
        }

        // Wait for the threads to complete any remaining work
        for child in children {
            child.join().expect("oops! the child thread panicked");
        }

        // let sub_tasks: &mut Vec<SubTask> = task.sub_tasks.unwrap().as_mut();
        // for sub_task_record in &sub_tasks_from_threads {
        //     let position_of_sub_task = sub_tasks
        //         .iter()
        //         .position(|sub_task| sub_task_record.id == sub_task.id)
        //         .unwrap();
        //     let sub_task = sub_tasks.get_mut(position_of_sub_task).unwrap();
        //     sub_task.task = Some(sub_task_record.clone());
        // }

        // dbg!(&sub_tasks_from_threads);
    };

    // dbg!(&task);

    task
}
