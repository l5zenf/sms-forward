#[derive(Debug, Clone)]
pub struct ModemStatus {
    pub sim_ready: bool,
    pub registered: bool,
    pub roaming: bool,
    pub csq: Option<i32>,
    pub rssi_dbm: Option<i32>,
    pub operator: Option<String>,
    pub last_error: Option<String>,
}
