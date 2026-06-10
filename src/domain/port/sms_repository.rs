use async_trait::async_trait;
use serde::{Deserialize, Serialize};

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

    // ── Read-only queries (HTTP / Web UI) ───────────────────────────────

    /// Paginated + filtered list of messages, newest first.
    ///
    /// `status` filters by exact status ("pending" / "sending" / "sent" /
    /// "failed" / "decode_failed"); `None` returns every status.
    /// `query` is a case-insensitive substring matched against sender and
    /// content; `None` skips the text filter.
    async fn list_messages(
        &self,
        filter: MessageFilter,
    ) -> Result<MessagePage, AppError>;

    async fn get_message(&self, id: i64) -> Result<Option<SmsMessage>, AppError>;

    /// Per-status counts for the dashboard. Counts every row regardless of
    /// status text (unknown statuses are folded into `other`).
    async fn count_by_status(&self) -> Result<StatusCounts, AppError>;

    async fn latest_modem_status(&self) -> Result<Option<ModemStatusRecord>, AppError>;

    async fn recent_modem_events(
        &self,
        limit: u64,
    ) -> Result<Vec<ModemEventRecord>, AppError>;
}

/// Query parameters for [SmsRepository::list_messages].
#[derive(Debug, Clone, Default)]
pub struct MessageFilter {
    pub limit: u64,
    pub offset: u64,
    pub status: Option<String>,
    pub query: Option<String>,
}

/// Result of a paginated listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePage {
    pub items: Vec<SmsMessage>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusCounts {
    pub pending: u64,
    pub sending: u64,
    pub sent: u64,
    pub failed: u64,
    pub decode_failed: u64,
    pub other: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModemStatusRecord {
    pub sim_ready: bool,
    pub registered: bool,
    pub roaming: bool,
    pub csq: Option<i32>,
    pub rssi_dbm: Option<i32>,
    pub operator: Option<String>,
    pub last_error: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModemEventRecord {
    pub id: i64,
    pub event_type: String,
    pub payload: String,
    pub created_at: Option<String>,
}
