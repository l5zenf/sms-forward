use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsMessage {
    pub id: i64,
    pub iccid: Option<String>,
    pub sender: Option<String>,
    pub content: Option<String>,
    pub sms_time: Option<String>,
    pub received_at: String,
    pub pdu_raw: String,
    pub dcs: Option<i32>,
    pub encoding: Option<String>,
    pub concat_ref: Option<String>,
    pub concat_total: Option<i32>,
    pub concat_completed: i32,
    pub modem_mem: Option<String>,
    pub modem_index: Option<i32>,
    pub dedupe_key: String,
    pub status: String,
    pub retry_count: i32,
    pub max_retry: i32,
    pub next_retry_at: Option<String>,
    pub locked_at: Option<String>,
    pub locked_by: Option<String>,
    pub forwarded_at: Option<String>,
    pub forward_response: Option<String>,
    pub last_error: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewSmsMessage {
    pub iccid: Option<String>,
    pub sender: Option<String>,
    pub content: String,
    pub sms_time: Option<String>,
    pub received_at: String,
    pub pdu_raw: String,
    pub dcs: Option<i32>,
    pub encoding: Option<String>,
    pub modem_mem: Option<String>,
    pub modem_index: Option<i32>,
    pub dedupe_key: String,
}
