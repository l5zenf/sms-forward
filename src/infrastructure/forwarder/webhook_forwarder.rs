//! WebhookForwarder — POST 转 SMS 到配置 URL；2xx 视为成功，其他视为失败.
//! 详见 plain.txt §11.

use async_trait::async_trait;
use serde::Serialize;
use tracing::{debug, warn};

use crate::domain::error::AppError;
use crate::domain::model::ForwardResult;
use crate::domain::model::sms_message::SmsMessage;
use crate::domain::port::forwarder_port::ForwarderPort;

pub struct WebhookForwarder {
    client: reqwest::Client,
    url: String,
}

#[derive(Serialize)]
struct WebhookPayload<'a> {
    id: i64,
    sender: Option<&'a str>,
    content: Option<&'a str>,
    sms_time: Option<&'a str>,
    received_at: &'a str,
    encoding: Option<&'a str>,
    dedupe_key: &'a str,
    retry_count: i32,
}

impl WebhookForwarder {
    pub fn new(url: String, timeout_secs: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .expect("failed to build reqwest client");
        Self { client, url }
    }
}

#[async_trait]
impl ForwarderPort for WebhookForwarder {
    async fn forward(&self, sms: &SmsMessage) -> Result<ForwardResult, AppError> {
        let payload = WebhookPayload {
            id: sms.id,
            sender: sms.sender.as_deref(),
            content: sms.content.as_deref(),
            sms_time: sms.sms_time.as_deref(),
            received_at: &sms.received_at,
            encoding: sms.encoding.as_deref(),
            dedupe_key: &sms.dedupe_key,
            retry_count: sms.retry_count,
        };

        debug!(id = sms.id, url = %self.url, "forwarding SMS via webhook");

        let resp = self
            .client
            .post(&self.url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::Forward {
                target: self.url.clone(),
                source: e,
            })?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();

        if status.is_success() {
            debug!(id = sms.id, status = %status, "webhook success");
            Ok(ForwardResult {
                success: true,
                response: format!("{} {}", status.as_u16(), body),
            })
        } else {
            warn!(id = sms.id, status = %status, "webhook non-2xx");
            Ok(ForwardResult {
                success: false,
                response: format!("{} {}", status.as_u16(), body),
            })
        }
    }
}
