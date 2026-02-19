#![allow(dead_code)]

use crate::services::clickup::{ClickUpTaskResponseBody, IN_PROGRESS_ORDER_INDEX};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Task {
    number: String,
    name: String,
    points: f32,
    total_points: f32,
    time_in_dev_status: i64,
    total_time_in_dev_status: i64,
    sub_tasks: Vec<Task>,
}

impl From<ClickUpTaskResponseBody> for Task {
    fn from(value: ClickUpTaskResponseBody) -> Self {
        let time_in_dev_status = get_days_in_dev_status(&value);

        let total_time_in_dev_status = match &value.sub_tasks {
            Some(sub_tasks) => sub_tasks.iter().fold(0, |acc, t| {
                if let Some(task) = &t.task {
                    return acc + get_days_in_dev_status(task);
                }

                acc
            }),
            None => 0,
        } + time_in_dev_status;

        let sub_tasks: Vec<Task> = match value.sub_tasks {
            Some(sub_tasks) => sub_tasks
                .iter()
                .filter_map(|t| {
                    if let Some(sub_task) = &t.task {
                        return Some(Task::from(sub_task.to_owned()));
                    }

                    None
                })
                .collect(),
            None => vec![],
        };

        let points = value.points.unwrap_or_default();

        let total_points = sub_tasks.iter().fold(points, |acc, t| acc + t.total_points);

        let number = match value.custom_id {
            Some(number) => number,
            None => value.id,
        };

        Self {
            number,
            name: value.name,
            points,
            total_points,
            time_in_dev_status,
            total_time_in_dev_status,
            sub_tasks,
        }
    }
}

pub enum TimeInStatusFormula {
    FullTime,
    NoWeekends,
}

pub fn apply_no_weekends_formula(task: &mut Task) {
    task.time_in_dev_status = (task.time_in_dev_status as f64 * 0.7142857143).ceil() as i64;
    task.total_time_in_dev_status =
        (task.total_time_in_dev_status as f64 * 0.7142857143).ceil() as i64;

    for task in &mut task.sub_tasks {
        apply_no_weekends_formula(task);
    }
}

pub fn generate_points_vs_time_spent_analysis(task: &Task) -> String {
    fn generate_points_vs_time_spent_analysis_iter(task: &Task, mut prefix: String) -> String {
        let mut result = format!(
            "\n{prefix}{} {} - points: {} ({}), time_spent: {} ({})",
            task.number,
            task.name,
            task.points,
            task.total_points,
            task.time_in_dev_status,
            task.total_time_in_dev_status,
        );

        prefix.push('\t');

        for sub_task in &task.sub_tasks {
            let nested_result =
                generate_points_vs_time_spent_analysis_iter(sub_task, prefix.clone());
            result.push_str(&nested_result);
        }

        result
    }

    generate_points_vs_time_spent_analysis_iter(task, "".to_string())
}

fn get_days_in_dev_status(task: &ClickUpTaskResponseBody) -> i64 {
    let total = task
        .time_in_status
        .as_ref()
        .unwrap()
        .status_history
        .iter()
        .filter_map(|s| match &s.order_index {
            Some(order_index) => {
                if *order_index >= IN_PROGRESS_ORDER_INDEX {
                    Some(s.total_time.by_minute / 60 / 24)
                } else {
                    None
                }
            }
            None => None,
        })
        .reduce(|acc, n| acc + n)
        .unwrap_or_default();

    // (total as f64 * 0.7142857143).ceil() as i64
    total
}
