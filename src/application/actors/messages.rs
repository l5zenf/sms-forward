//! Kameo message types for inter-actor communication, aligned with plain.txt §10.

use kameo::Actor;

use crate::domain::model::ModemStatus;
use crate::domain::port::modem_port::RawSmsPdu;

/// +CMTI 通知 → AtActor 调 CMGR → 把 raw_pdu 发给 SmsIngestActor 解码入库
#[derive(Debug, Clone)]
pub struct RawSmsPduReceived {
    pub mem: String,
    pub index: i32,
    pub raw_pdu: String,
}

/// SmsIngestActor 入库后回发给 AtActor，触发 CMGD
#[derive(Debug, Clone)]
pub struct SmsPersisted {
    pub mem: String,
    pub index: i32,
}

/// HealthActor → AtActor 查询 modem 状态
#[derive(Debug, Clone, Actor)]
pub struct QueryStatus;

/// ForwarderActor 周期 tick
#[derive(Debug, Clone)]
pub struct ForwardTick;

/// ReaperActor 周期 tick
#[derive(Debug, Clone)]
pub struct RecoverStaleSending;

/// HealthActor 周期 tick
#[derive(Debug, Clone)]
pub struct HealthTick;

/// 转发链路返回的状态
#[derive(Debug, Clone)]
pub struct ModemStatusReply(pub ModemStatus);
