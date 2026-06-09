use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, DbBackend, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder, Set, Statement, TransactionTrait,
};
use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::domain::error::AppError;
use crate::domain::model::sms_message::{NewSmsMessage, SmsMessage};
use crate::domain::model::sms_part::{MultipartKey, NewSmsPart};
use crate::domain::port::sms_repository::SmsRepository;
use crate::infrastructure::persistence::entity::{sms_message, sms_part};

pub struct SeaOrmSmsRepository {
    db: DatabaseConnection,
    worker_id: String,
}

impl SeaOrmSmsRepository {
    pub fn new(db: DatabaseConnection, worker_id: String) -> Self {
        Self { db, worker_id }
    }

    fn hash_key(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn now_str() -> String {
        chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string()
    }

    fn db_err(op: &str, e: sea_orm::DbErr) -> AppError {
        AppError::Database {
            op: op.into(),
            source: e,
        }
    }
}

#[async_trait]
impl SmsRepository for SeaOrmSmsRepository {
    async fn save_single_message(&self, sms: NewSmsMessage) -> Result<i64, AppError> {
        let now = Self::now_str();
        let active = sms_message::ActiveModel {
            iccid: Set(sms.iccid),
            sender: Set(sms.sender),
            content: Set(Some(sms.content)),
            sms_time: Set(sms.sms_time),
            received_at: Set(now.clone()),
            pdu_raw: Set(sms.pdu_raw),
            dcs: Set(sms.dcs),
            encoding: Set(sms.encoding),
            concat_ref: Set(None),
            concat_total: Set(None),
            concat_completed: Set(1),
            modem_mem: Set(sms.modem_mem),
            modem_index: Set(sms.modem_index),
            dedupe_key: Set(sms.dedupe_key),
            status: Set("pending".into()),
            retry_count: Set(0),
            max_retry: Set(10),
            next_retry_at: Set(Some(now)),
            ..Default::default()
        };

        let model = active
            .insert(&self.db)
            .await
            .map_err(|e| Self::db_err("save_single_message", e))?;

        Ok(model.id)
    }

    async fn save_part(&self, part: NewSmsPart) -> Result<(), AppError> {
        let active = sms_part::ActiveModel {
            iccid: Set(part.iccid),
            sender: Set(part.sender),
            sms_time: Set(part.sms_time),
            concat_ref: Set(part.concat_ref),
            concat_total: Set(part.concat_total),
            concat_seq: Set(part.concat_seq),
            pdu_raw: Set(part.pdu_raw),
            decoded_content: Set(part.decoded_content),
            dcs: Set(part.dcs),
            encoding: Set(part.encoding),
            modem_mem: Set(part.modem_mem),
            modem_index: Set(part.modem_index),
            part_dedupe_key: Set(part.part_dedupe_key),
            ..Default::default()
        };

        active
            .insert(&self.db)
            .await
            .map_err(|e| Self::db_err("save_part", e))?;

        Ok(())
    }

    async fn try_assemble_multipart(
        &self,
        key: &MultipartKey,
    ) -> Result<Option<i64>, AppError> {
        let parts = sms_part::Entity::find()
            .filter(sms_part::Column::Sender.eq(&key.sender))
            .filter(sms_part::Column::ConcatRef.eq(&key.concat_ref))
            .filter(sms_part::Column::ConcatTotal.eq(key.concat_total))
            .order_by_asc(sms_part::Column::ConcatSeq)
            .all(&self.db)
            .await
            .map_err(|e| Self::db_err("try_assemble_multipart query", e))?;

        if parts.len() as i32 != key.concat_total {
            return Ok(None);
        }

        let mut merged_content = String::new();
        let mut all_pdu_raw = Vec::new();
        let first = parts.first().unwrap();

        for p in &parts {
            merged_content.push_str(&p.decoded_content);
            all_pdu_raw.push(p.pdu_raw.clone());
        }

        let dedupe_input = format!(
            "{}|{}|{}|{}|{}",
            first.iccid.as_deref().unwrap_or(""),
            first.sender,
            first.sms_time.as_deref().unwrap_or(""),
            first.concat_ref,
            merged_content
        );
        let dedupe_key = Self::hash_key(&dedupe_input);
        let now = Self::now_str();

        let existing = sms_message::Entity::find()
            .filter(sms_message::Column::DedupeKey.eq(&dedupe_key))
            .one(&self.db)
            .await
            .map_err(|e| Self::db_err("try_assemble_multipart dedupe check", e))?;

        if existing.is_some() {
            return Ok(None);
        }

        let active = sms_message::ActiveModel {
            iccid: Set(first.iccid.clone()),
            sender: Set(Some(first.sender.clone())),
            content: Set(Some(merged_content)),
            sms_time: Set(first.sms_time.clone()),
            received_at: Set(now.clone()),
            pdu_raw: Set(all_pdu_raw.join("|")),
            dcs: Set(first.dcs),
            encoding: Set(first.encoding.clone()),
            concat_ref: Set(Some(key.concat_ref.clone())),
            concat_total: Set(Some(key.concat_total)),
            concat_completed: Set(1),
            modem_mem: Set(first.modem_mem.clone()),
            modem_index: Set(first.modem_index),
            dedupe_key: Set(dedupe_key),
            status: Set("pending".into()),
            retry_count: Set(0),
            max_retry: Set(10),
            next_retry_at: Set(Some(now)),
            ..Default::default()
        };

        let model = active
            .insert(&self.db)
            .await
            .map_err(|e| Self::db_err("try_assemble_multipart insert", e))?;

        info!(id = model.id, "assembled multipart SMS");
        Ok(Some(model.id))
    }

    async fn save_decode_failed(
        &self,
        iccid: Option<String>,
        raw_pdu: String,
        modem_mem: Option<String>,
        modem_index: Option<i32>,
        error: String,
    ) -> Result<i64, AppError> {
        let now = Self::now_str();
        let dedupe_input = format!("decode_failed|{}|{}", raw_pdu, now);
        let dedupe_key = Self::hash_key(&dedupe_input);

        let active = sms_message::ActiveModel {
            iccid: Set(iccid),
            pdu_raw: Set(raw_pdu),
            modem_mem: Set(modem_mem),
            modem_index: Set(modem_index),
            dedupe_key: Set(dedupe_key),
            status: Set("decode_failed".into()),
            received_at: Set(now.clone()),
            next_retry_at: Set(Some(now)),
            last_error: Set(Some(error)),
            max_retry: Set(0),
            ..Default::default()
        };

        let model = active
            .insert(&self.db)
            .await
            .map_err(|e| Self::db_err("save_decode_failed", e))?;

        Ok(model.id)
    }

    async fn claim_next_pending(
        &self,
        worker_id: &str,
    ) -> Result<Option<SmsMessage>, AppError> {
        let txn: DatabaseTransaction = self
            .db
            .begin()
            .await
            .map_err(|e| Self::db_err("claim_next_pending begin", e))?;

        let result = sms_message::Entity::find()
            .filter(sms_message::Column::Status.eq("pending"))
            .filter(sms_message::Column::NextRetryAt.lte(Self::now_str()))
            .order_by_asc(sms_message::Column::Id)
            .one(&txn)
            .await
            .map_err(|e| Self::db_err("claim_next_pending query", e))?;

        let Some(model) = result else {
            txn.commit()
                .await
                .map_err(|e| Self::db_err("claim_next_pending commit (none)", e))?;
            return Ok(None);
        };

        let now = Self::now_str();
        let mut active = model.clone().into_active_model();
        active.status = Set("sending".into());
        active.locked_at = Set(Some(now));
        active.locked_by = Set(Some(worker_id.to_string()));

        active
            .update(&txn)
            .await
            .map_err(|e| Self::db_err("claim_next_pending update", e))?;

        txn.commit()
            .await
            .map_err(|e| Self::db_err("claim_next_pending commit", e))?;

        Ok(Some(entity_to_domain(model)))
    }

    async fn mark_sent(&self, id: i64, response: String) -> Result<(), AppError> {
        let now = Self::now_str();
        let model = sms_message::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| Self::db_err("mark_sent find", e))?;

        let Some(model) = model else {
            return Err(AppError::Database {
                op: format!("mark_sent: message {id} not found"),
                source: sea_orm::DbErr::RecordNotFound(format!("id={id}")),
            });
        };

        let mut active = model.into_active_model();
        active.status = Set("sent".into());
        active.forwarded_at = Set(Some(now.clone()));
        active.forward_response = Set(Some(response));
        active.updated_at = Set(Some(now));
        active.locked_at = Set(None);
        active.locked_by = Set(None);

        active
            .update(&self.db)
            .await
            .map_err(|e| Self::db_err("mark_sent", e))?;

        Ok(())
    }

    async fn mark_failed(&self, id: i64, error: String) -> Result<(), AppError> {
        let now = Self::now_str();
        let model = sms_message::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| Self::db_err("mark_failed find", e))?;

        let Some(model) = model else {
            return Err(AppError::Database {
                op: format!("mark_failed: message {id} not found"),
                source: sea_orm::DbErr::RecordNotFound(format!("id={id}")),
            });
        };

        let new_retry = model.retry_count + 1;
        let max_retry = model.max_retry;
        let mut active = model.into_active_model();
        active.retry_count = Set(new_retry);
        active.last_error = Set(Some(error));
        active.updated_at = Set(Some(now.clone()));
        active.locked_at = Set(None);
        active.locked_by = Set(None);

        if new_retry >= max_retry {
            active.status = Set("failed".into());
            warn!(id = id, retry = new_retry, "message failed permanently");
        } else {
            active.status = Set("pending".into());
            let delay = (60u64).saturating_mul(new_retry as u64).min(3600);
            let next = chrono::Utc::now() + chrono::Duration::seconds(delay as i64);
            let next_str = next.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
            active.next_retry_at = Set(Some(next_str));
            info!(id = id, retry = new_retry, delay_secs = delay, "retry scheduled");
        }

        active
            .update(&self.db)
            .await
            .map_err(|e| Self::db_err("mark_failed", e))?;

        Ok(())
    }

    async fn recover_stale_sending(&self) -> Result<u64, AppError> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::seconds(300))
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();

        let sql = format!(
            "UPDATE sms_messages SET status = 'pending', locked_at = NULL, locked_by = NULL \
             WHERE status = 'sending' AND locked_at < '{}'",
            cutoff
        );

        let result = self
            .db
            .execute(Statement::from_string(DbBackend::Sqlite, sql))
            .await
            .map_err(|e| Self::db_err("recover_stale_sending", e))?;

        let count = result.rows_affected();
        if count > 0 {
            info!(count = count, "recovered stale sending messages");
        }
        Ok(count)
    }
}

fn entity_to_domain(m: sms_message::Model) -> SmsMessage {
    SmsMessage {
        id: m.id,
        iccid: m.iccid,
        sender: m.sender,
        content: m.content,
        sms_time: m.sms_time,
        received_at: m.received_at,
        pdu_raw: m.pdu_raw,
        dcs: m.dcs,
        encoding: m.encoding,
        concat_ref: m.concat_ref,
        concat_total: m.concat_total,
        concat_completed: m.concat_completed,
        modem_mem: m.modem_mem,
        modem_index: m.modem_index,
        dedupe_key: m.dedupe_key,
        status: m.status,
        retry_count: m.retry_count,
        max_retry: m.max_retry,
        next_retry_at: m.next_retry_at,
        locked_at: m.locked_at,
        locked_by: m.locked_by,
        forwarded_at: m.forwarded_at,
        forward_response: m.forward_response,
        last_error: m.last_error,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}
