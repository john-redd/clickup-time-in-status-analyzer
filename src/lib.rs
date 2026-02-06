use crate::services::clickup::ClickUpService;
use std::sync::Arc;

pub mod domain;
pub mod routes;
pub mod services;

#[derive(Clone)]
pub struct AppState {
    pub click_up_service: Arc<ClickUpService>
}
