use crate::domain::error::AppError;

/// Result of decoding a raw PDU
#[derive(Debug, Clone)]
pub struct DecodedPdu {
    pub sender: Option<String>,
    pub content: String,
    pub sms_time: Option<String>,
    pub dcs: u8,
    pub encoding: Option<String>,
    /// If this is part of a multipart SMS
    pub udh: Option<PduUdh>,
}

#[derive(Debug, Clone)]
pub struct PduUdh {
    pub concat_ref: String,
    pub concat_total: u8,
    pub concat_seq: u8,
}

pub trait PduDecoder: Send + Sync {
    fn decode(&self, raw_pdu: &str) -> Result<DecodedPdu, AppError>;
}
