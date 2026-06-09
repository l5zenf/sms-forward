//! SmsIngestActor — 接收 RawSmsPduReceived，解码 PDU，入库，回发 SmsPersisted。
//! 详见 plain.txt §10。

use std::sync::Arc;

use kameo::actor::{Actor, ActorRef};
use kameo::mailbox::unbounded::UnboundedMailbox;
use kameo::message::{Context, Message};
use kameo::Reply;
use sha2::{Digest, Sha256};
use tracing::{error, info, warn};

use crate::application::actors::at_actor::AtActor;
use crate::application::actors::messages::{RawSmsPduReceived, SmsPersisted};
use crate::domain::error::AppError;
use crate::domain::model::sms_message::NewSmsMessage;
use crate::domain::model::sms_part::{MultipartKey, NewSmsPart};
use crate::domain::port::pdu_decoder::PduDecoder;
use crate::domain::port::sms_repository::SmsRepository;
use crate::infrastructure::modem::pdu_decoder::DefaultPduDecoder;

pub struct SmsIngestActor {
    repo: Arc<dyn SmsRepository>,
    pdu_decoder: Arc<dyn PduDecoder>,
    at_actor_ref: ActorRef<AtActor>,
}

impl SmsIngestActor {
    pub fn new(repo: Arc<dyn SmsRepository>, at_ref: ActorRef<AtActor>) -> Self {
        Self {
            repo,
            pdu_decoder: Arc::new(DefaultPduDecoder),
            at_actor_ref: at_ref,
        }
    }

    fn hash_key(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }
}

impl Actor for SmsIngestActor {
    type Mailbox = UnboundedMailbox<Self>;

    async fn on_start(&mut self, _actor_ref: ActorRef<Self>) -> Result<(), kameo::error::BoxError> {
        info!("SmsIngestActor starting");
        Ok(())
    }
}

/// Reply for tell messages — represents "we persisted this raw_pdu (or saved decode_failed)"
#[derive(Debug, Clone, Reply)]
pub struct PersistedAck(pub bool);

impl Message<RawSmsPduReceived> for SmsIngestActor {
    type Reply = PersistedAck;

    async fn handle(
        &mut self,
        msg: RawSmsPduReceived,
        _ctx: Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        info!(
            mem = %msg.mem,
            index = msg.index,
            pdu_len = msg.raw_pdu.len(),
            "[INGEST] received raw PDU"
        );
        match self.process_pdu(msg.clone()).await {
            Ok(()) => {
                let persisted = SmsPersisted {
                    mem: msg.mem.clone(),
                    index: msg.index,
                };
                info!(
                    mem = %msg.mem,
                    index = msg.index,
                    "[INGEST] persisted OK, telling AT to delete"
                );
                if let Err(e) = self.at_actor_ref.tell(persisted).await {
                    error!(error = %e, "[INGEST] failed to send SmsPersisted to AtActor");
                }
                PersistedAck(true)
            }
            Err(AppError::Database { op, source }) => {
                error!(op = %op, error = %source, "[INGEST] DB write failed; will NOT delete SIM SMS");
                PersistedAck(false)
            }
            Err(e) => {
                error!(error = %e, "[INGEST] ingest failed; will NOT delete SIM SMS");
                PersistedAck(false)
            }
        }
    }
}

impl SmsIngestActor {
    async fn process_pdu(&self, msg: RawSmsPduReceived) -> Result<(), AppError> {
        let decoded = match self.pdu_decoder.decode(&msg.raw_pdu) {
            Ok(d) => d,
            Err(e) => {
                warn!(error = %e, raw_pdu = %msg.raw_pdu, "[INGEST] PDU decode failed, saving raw");
                self.repo
                    .save_decode_failed(
                        None,
                        msg.raw_pdu.clone(),
                        Some(msg.mem.clone()),
                        Some(msg.index),
                        format!("{e}"),
                    )
                    .await?;
                return Ok(());
            }
        };
        info!(
            sender = decoded.sender.as_deref().unwrap_or("?"),
            encoding = decoded.encoding.as_deref().unwrap_or("?"),
            dcs = decoded.dcs,
            content_len = decoded.content.chars().count(),
            has_udh = decoded.udh.is_some(),
            sms_time = decoded.sms_time.as_deref().unwrap_or("?"),
            "[INGEST] PDU decoded"
        );

        if let Some(udh) = &decoded.udh {
            // multipart: save part + try assemble
            let sender = decoded.sender.clone().unwrap_or_else(|| "unknown".to_string());
            let part = NewSmsPart {
                iccid: None,
                sender: sender.clone(),
                sms_time: decoded.sms_time.clone(),
                concat_ref: udh.concat_ref.clone(),
                concat_total: udh.concat_total as i32,
                concat_seq: udh.concat_seq as i32,
                pdu_raw: msg.raw_pdu.clone(),
                decoded_content: decoded.content.clone(),
                dcs: Some(decoded.dcs as i32),
                encoding: decoded.encoding.clone(),
                modem_mem: Some(msg.mem.clone()),
                modem_index: Some(msg.index),
                part_dedupe_key: Self::hash_key(&format!(
                    "{}|{}|{}|{}|{}|{}",
                    sender,
                    decoded.sms_time.as_deref().unwrap_or(""),
                    udh.concat_ref,
                    udh.concat_total,
                    udh.concat_seq,
                    msg.raw_pdu
                )),
            };
            // Try insert (ignore UNIQUE constraint failure — duplication is OK)
            if let Err(e) = self.repo.save_part(part).await {
                warn!(error = %e, "save_part failed (likely duplicate, ignoring)");
            }

            let key = MultipartKey {
                sender,
                concat_ref: udh.concat_ref.clone(),
                concat_total: udh.concat_total as i32,
            };
            self.repo.try_assemble_multipart(&key).await?;
            info!(concat_ref = %udh.concat_ref, total = udh.concat_total, seq = udh.concat_seq, "[INGEST] multipart part saved (may have completed)");
        } else {
            // single message
            let sender = decoded.sender.clone().unwrap_or_else(|| "unknown".to_string());
            let dedupe_input = format!(
                "{}|{}|{}|{}",
                sender,
                decoded.sms_time.as_deref().unwrap_or(""),
                decoded.content,
                msg.raw_pdu
            );
            let new_sms = NewSmsMessage {
                iccid: None,
                sender: Some(sender),
                content: decoded.content,
                sms_time: decoded.sms_time,
                received_at: now_str(),
                pdu_raw: msg.raw_pdu,
                dcs: Some(decoded.dcs as i32),
                encoding: decoded.encoding,
                modem_mem: Some(msg.mem),
                modem_index: Some(msg.index),
                dedupe_key: Self::hash_key(&dedupe_input),
            };
            match self.repo.save_single_message(new_sms).await {
                Ok(id) => info!(id = id, "[INGEST] saved single SMS"),
                Err(AppError::Database { source, .. })
                    if source.to_string().contains("UNIQUE") =>
                {
                    info!("[INGEST] duplicate SMS (dedupe hit), ignoring");
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

fn now_str() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}
