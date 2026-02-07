#![allow(dead_code)]

use crate::services::clickup::{ClickUpTaskResponseBody, IN_PROGRESS_ORDER_INDEX};
use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct Ticket {
    number: String,
    name: String,
    points: f32,
    total_points: f32,
    time_in_dev_status: i64,
    total_time_in_dev_status: i64,
    sub_tickets: Vec<Ticket>,
}

impl From<ClickUpTaskResponseBody> for Ticket {
    fn from(value: ClickUpTaskResponseBody) -> Self {
        let time_in_dev_status = get_days_in_dev_status(&value);

        let total_time_in_dev_status = match &value.sub_tasks {
            Some(sub_tasks) => sub_tasks.iter().fold(time_in_dev_status, |acc, t| {
                if let Some(task) = &t.task {
                    return acc + get_days_in_dev_status(task);
                }

                acc
            }),
            None => 0,
        } + time_in_dev_status;

        let sub_tickets: Vec<Ticket> = match value.sub_tasks {
            Some(sub_tickets) => sub_tickets
                .iter()
                .filter_map(|t| {
                    if let Some(sub_task) = &t.task {
                        return Some(Ticket::from(sub_task.to_owned()));
                    }

                    None
                })
                .collect(),
            None => vec![],
        };

        let points = value.points.unwrap_or_default();

        let total_points = sub_tickets
            .iter()
            .fold(points, |acc, t| acc + t.total_points);

        Self {
            number: value.custom_id,
            name: value.name,
            points,
            total_points,
            time_in_dev_status,
            total_time_in_dev_status,
            sub_tickets,
        }
    }
}

pub fn generate_points_vs_time_spent_analysis(task: &Ticket) -> String {
    fn generate_points_vs_time_spent_analysis_iter(task: &Ticket, mut prefix: String) -> String {
        let mut result = format!(
            "\n{prefix}{} - points: {} ({}), time_spent: {} ({})",
            task.number,
            task.points,
            task.total_points,
            task.time_in_dev_status,
            task.total_time_in_dev_status,
        );

        prefix.push('\t');

        for sub_task in &task.sub_tickets {
            let nested_result =
                generate_points_vs_time_spent_analysis_iter(sub_task, prefix.clone());
            result.push_str(&nested_result);
        }

        result
    }

    generate_points_vs_time_spent_analysis_iter(task, "".to_string())
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

    (total as f64 * 0.7142857143).ceil() as i64
}
