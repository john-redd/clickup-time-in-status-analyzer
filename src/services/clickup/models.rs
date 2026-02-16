use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize, Clone)]
pub struct ClickUpTimeInStatusResponseBody {
    pub current_status: CurrentStatus,
    pub status_history: Vec<StatusHistory>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CurrentStatus {
    pub status: String,
    pub total_time: TotalTime,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TotalTime {
    pub by_minute: i64,
    #[serde(deserialize_with = "ts_milliseconds_string")]
    pub since: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StatusHistory {
    pub status: String,
    #[serde(rename = "type")]
    pub status_type: String,
    pub total_time: TotalTime,
    #[serde(rename = "orderindex")]
    pub order_index: Option<i32>,
}

fn ts_milliseconds_string<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let val = String::deserialize(deserializer)?;
    let val = val.parse::<i64>().unwrap(); // TODO: Fix this unwrap.
    let date = DateTime::from_timestamp_millis(val).unwrap().to_utc();
    Ok(date)
}

#[derive(Debug, Deserialize, Clone)]
pub struct ClickUpTaskResponseBody {
    pub id: String,
    pub custom_id: Option<String>,
    pub name: String,
    pub text_content: String,
    pub description: String,
    pub points: Option<f32>,
    #[serde(deserialize_with = "ts_milliseconds_string")]
    pub date_created: DateTime<Utc>,
    #[serde(rename = "subtasks")]
    pub sub_tasks: Option<Vec<SubTask>>,
    pub time_in_status: Option<ClickUpTimeInStatusResponseBody>, // Not actually part of request.
}

#[derive(Debug, Deserialize, Clone)]
pub struct SubTask {
    pub id: String,
    pub task: Option<ClickUpTaskResponseBody>, // Not actually part of response
    pub custom_id: Option<String>,
    pub name: String,
    pub points: Option<f32>,
    #[serde(deserialize_with = "ts_milliseconds_string")]
    pub date_created: DateTime<Utc>,
}
