use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sms_messages")]
pub struct Model {
    #[sea_orm(primary_key)]
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

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
