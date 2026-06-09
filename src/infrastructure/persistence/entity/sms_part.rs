use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sms_parts")]
pub struct Model {
    #[sea_orm(primary_key)]
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

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
