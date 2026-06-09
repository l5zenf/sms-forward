use async_trait::async_trait;

use crate::domain::error::AppError;
use crate::domain::model::ForwardResult;
use crate::domain::model::sms_message::SmsMessage;

#[async_trait]
pub trait ForwarderPort: Send + Sync {
    async fn forward(&self, sms: &SmsMessage) -> Result<ForwardResult, AppError>;
}
