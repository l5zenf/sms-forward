use async_trait::async_trait;

use crate::domain::error::AppError;
use crate::domain::model::ModemStatus;

/// Raw PDU received from the modem before decoding
#[derive(Debug, Clone)]
pub struct RawSmsPdu {
    pub mem: String,
    pub index: i32,
    pub raw_pdu: String,
}

/// Events emitted by the modem (AT responses and URCs)
#[derive(Debug, Clone)]
pub enum ModemEvent {
    NewSms(RawSmsPdu),
    Ring,
    Clip(String),
    Ready,
    Error(String),
}

#[async_trait]
pub trait ModemPort: Send + Sync {
    async fn init(&self) -> Result<(), AppError>;

    async fn next_event(&self) -> Result<ModemEvent, AppError>;

    async fn read_sms_pdu(&self, mem: &str, index: i32) -> Result<RawSmsPdu, AppError>;

    async fn delete_sms(&self, mem: &str, index: i32) -> Result<(), AppError>;

    async fn query_status(&self) -> Result<ModemStatus, AppError>;
}
