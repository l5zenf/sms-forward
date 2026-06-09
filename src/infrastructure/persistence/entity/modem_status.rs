use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "modem_status")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub sim_ready: i32,
    pub registered: i32,
    pub roaming: i32,
    pub csq: Option<i32>,
    pub rssi_dbm: Option<i32>,
    pub operator: Option<String>,
    pub last_error: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
