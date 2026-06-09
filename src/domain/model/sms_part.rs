#[derive(Debug, Clone)]
pub struct SmsPart {
    pub id: i64,
    pub iccid: Option<String>,
    pub sender: String,
    pub sms_time: Option<String>,
    pub concat_ref: String,
    pub concat_total: i32,
    pub concat_seq: i32,
    pub pdu_raw: String,
    pub decoded_content: String,
    pub dcs: Option<i32>,
    pub encoding: Option<String>,
    pub modem_mem: Option<String>,
    pub modem_index: Option<i32>,
    pub part_dedupe_key: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewSmsPart {
    pub iccid: Option<String>,
    pub sender: String,
    pub sms_time: Option<String>,
    pub concat_ref: String,
    pub concat_total: i32,
    pub concat_seq: i32,
    pub pdu_raw: String,
    pub decoded_content: String,
    pub dcs: Option<i32>,
    pub encoding: Option<String>,
    pub modem_mem: Option<String>,
    pub modem_index: Option<i32>,
    pub part_dedupe_key: String,
}

/// Key to identify a multipart message group for assembly
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct MultipartKey {
    pub sender: String,
    pub concat_ref: String,
    pub concat_total: i32,
}
