use async_trait::async_trait;

use crate::domain::error::AppError;
use crate::domain::model::sms_message::{NewSmsMessage, SmsMessage};
use crate::domain::model::sms_part::{MultipartKey, NewSmsPart};

#[async_trait]
pub trait SmsRepository: Send + Sync {
    async fn save_single_message(&self, sms: NewSmsMessage) -> Result<i64, AppError>;

    async fn save_part(&self, part: NewSmsPart) -> Result<(), AppError>;

    async fn try_assemble_multipart(
        &self,
        key: &MultipartKey,
    ) -> Result<Option<i64>, AppError>;

    async fn save_decode_failed(
        &self,
        iccid: Option<String>,
        raw_pdu: String,
        modem_mem: Option<String>,
        modem_index: Option<i32>,
        error: String,
    ) -> Result<i64, AppError>;

    async fn claim_next_pending(
        &self,
        worker_id: &str,
    ) -> Result<Option<SmsMessage>, AppError>;

    async fn mark_sent(&self, id: i64, response: String) -> Result<(), AppError>;

    async fn mark_failed(&self, id: i64, error: String) -> Result<(), AppError>;

    async fn recover_stale_sending(&self) -> Result<u64, AppError>;
}
