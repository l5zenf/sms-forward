//! Shared application state injected into every axum handler.

use std::sync::Arc;

use crate::domain::port::sms_repository::SmsRepository;

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<dyn SmsRepository>,
}

impl AppState {
    pub fn new(repo: Arc<dyn SmsRepository>) -> Self {
        Self { repo }
    }
}
