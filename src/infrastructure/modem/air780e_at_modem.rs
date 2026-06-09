use async_trait::async_trait;
use tracing::{debug, error, info, trace, warn};

use crate::domain::error::AppError;
use crate::domain::model::ModemStatus;
use crate::domain::port::modem_port::{ModemEvent, ModemPort, RawSmsPdu};
use crate::infrastructure::modem::at_client::AtClient;
use crate::infrastructure::modem::at_parser::AtLine;

/// Steps in the modem initialization state machine
#[derive(Debug, Clone, PartialEq)]
enum InitStep {
    OpenSerial,
    ProbeAt,
    DisableEcho,
    EnableVerboseError,
    WaitSimReady,
    SetPduMode,
    SetStorage,
    SetNewMessageIndication,
    SetClip,
    InitialStatus,
    Ready,
}

pub struct Air780eAtModem {
    client: AtClient,
}

impl Air780eAtModem {
    pub fn new(port_path: String, baud_rate: u32, buffer_limit: usize) -> Self {
        Self {
            client: AtClient::new(port_path, baud_rate, buffer_limit),
        }
    }

    async fn send_at_ok(&self, cmd: &str, step: &str) -> Result<Vec<AtLine>, AppError> {
        let lines = self.client.send_command(cmd).await?;
        if AtClient::is_error(&lines) {
            let cme = AtClient::cme_error(&lines);
            error!(
                step = step,
                cmd = cmd,
                cme_code = cme.as_ref().map(|c| c.0).unwrap_or(0),
                "AT command returned error"
            );
            return Err(AppError::ModemInit {
                step: format!("{step}: {cmd}"),
            });
        }
        Ok(lines)
    }

    async fn execute_init_step(&self, step: &InitStep) -> Result<(), AppError> {
        let step_name = format!("{:?}", step);
        match step {
            InitStep::OpenSerial => {
                info!("opening serial port...");
                self.client.open().await?;
            }
            InitStep::ProbeAt => {
                info!("probing AT...");
                self.send_at_ok("AT", &step_name).await?;
            }
            InitStep::DisableEcho => {
                info!("disabling echo...");
                self.send_at_ok("ATE0", &step_name).await?;
            }
            InitStep::EnableVerboseError => {
                info!("enabling verbose errors...");
                self.send_at_ok("AT+CMEE=2", &step_name).await?;
            }
            InitStep::WaitSimReady => {
                info!("waiting for SIM ready...");
                for _ in 0..20 {
                    let lines = self.client.send_command("AT+CPIN?").await?;
                    for l in &lines {
                        if let AtLine::Cpin(status) = l {
                            if status.contains("READY") {
                                info!("SIM ready");
                                return Ok(());
                            }
                            debug!(status = %status, "SIM not ready yet");
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                }
                return Err(AppError::ModemInit {
                    step: "SIM not ready after 60s".into(),
                });
            }
            InitStep::SetPduMode => {
                info!("setting PDU mode...");
                self.send_at_ok("AT+CMGF=0", &step_name).await?;
            }
            InitStep::SetStorage => {
                info!("setting SMS storage...");
                self.send_at_ok(r#"AT+CPMS="SM","SM","SM""#, &step_name)
                    .await?;
            }
            InitStep::SetNewMessageIndication => {
                info!("setting new message indication...");
                self.send_at_ok("AT+CNMI=2,1,0,0,0", &step_name).await?;
            }
            InitStep::SetClip => {
                info!("enabling caller ID...");
                if let Err(e) = self.send_at_ok("AT+CLIP=1", &step_name).await {
                    warn!(error = %e, "AT+CLIP=1 rejected (incoming call ignored MVP-1); continuing");
                }
            }
            InitStep::InitialStatus => {
                info!("querying initial status...");
                let _ = self.client.send_command("AT+CSQ").await;
                let _ = self.client.send_command("AT+CREG?").await;
                let _ = self.client.send_command("AT+CEREG?").await;
                let _ = self.client.send_command("AT+COPS?").await;
            }
            InitStep::Ready => {
                info!("modem initialization complete");
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ModemPort for Air780eAtModem {
    async fn init(&self) -> Result<(), AppError> {
        let steps = [
            InitStep::OpenSerial,
            InitStep::ProbeAt,
            InitStep::DisableEcho,
            InitStep::EnableVerboseError,
            InitStep::WaitSimReady,
            InitStep::SetPduMode,
            InitStep::SetStorage,
            InitStep::SetNewMessageIndication,
            InitStep::SetClip,
            InitStep::InitialStatus,
            InitStep::Ready,
        ];

        for step in &steps {
            self.execute_init_step(step).await.map_err(|e| {
                error!(step = ?step, "init step failed");
                e
            })?;
        }

        Ok(())
    }

    async fn next_event(&self) -> Result<ModemEvent, AppError> {
        loop {
            match self.client.read_line().await? {
                None => {
                    // Both 0-bytes-available and Empty-line branches end up here.
                    // Yield so this loop doesn't hog the CPU while waiting.
                    tokio::task::yield_now().await;
                    continue;
                }
                Some(AtLine::Cmti { mem, index }) => {
                    info!(mem = %mem, index = index, "[AT] +CMTI URC parsed");
                    let raw = self.read_sms_pdu(&mem, index).await?;
                    return Ok(ModemEvent::NewSms(raw));
                }
                Some(AtLine::Ring) => {
                    info!("incoming call (RING)");
                    return Ok(ModemEvent::Ring);
                }
                Some(AtLine::Clip(data)) => {
                    info!(data = %data, "incoming call (CLIP)");
                    return Ok(ModemEvent::Clip(data));
                }
                Some(AtLine::Error) => {
                    warn!("unexpected ERROR from modem");
                    return Ok(ModemEvent::Error("unexpected ERROR".into()));
                }
                Some(AtLine::CmeError(code, msg)) => {
                    warn!(code = code, msg = %msg, "CME error from modem");
                    return Ok(ModemEvent::Error(format!("CME {}: {}", code, msg)));
                }
                _ => {
                    // No interesting event, sleep briefly to avoid busy-looping
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }
    }

    async fn read_sms_pdu(&self, mem: &str, index: i32) -> Result<RawSmsPdu, AppError> {
        let cmd = format!("AT+CMGR={}", index);
        info!(cmd = %cmd, "reading SMS PDU");

        let lines = self.client.send_command(&cmd).await?;

        if AtClient::is_error(&lines) {
            let cme = AtClient::cme_error(&lines);
            return Err(AppError::AtCommand {
                cmd: cmd.clone(),
                response: format!("CME error: {:?}", cme),
            });
        }

        // PDU response is in lines like:
        // +CMGR: <stat>,<length><CR><LF><pdu>
        // OK
        let mut pdu_lines: Vec<String> = Vec::new();
        let mut in_pdu = false;
        for line in &lines {
            match line {
                AtLine::Ok => break,
                AtLine::Data(s) if s.starts_with("+CMGR:") => {
                    in_pdu = true;
                    continue;
                }
                AtLine::Data(s) if in_pdu && s.chars().all(|c| c.is_ascii_hexdigit()) => {
                    pdu_lines.push(s.clone());
                }
                AtLine::Data(_) if in_pdu => {
                    // Could be the PDU itself (which is hex)
                    pdu_lines.push(String::new());
                    // The Data variant content was already captured
                }
                _ => continue,
            }
        }

        // The PDU data lines are hex strings after the +CMGR header
        // In practice, the CMGR response for PDU mode looks like:
        // +CMGR: 0,,26
        // 0791448720003023040C914477000000000000016060918183008000000000000000000000000000000000000000
        // OK

        // Find the PDU hex string (long hex line after +CMGR)
        let raw_pdu = lines
            .iter()
            .find_map(|l| {
                if let AtLine::Data(s) = l {
                    if !s.starts_with('+')
                        && s.len() > 10
                        && s.chars().all(|c| c.is_ascii_hexdigit())
                    {
                        Some(s.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap_or_default();

        if raw_pdu.is_empty() {
            warn!(cmd = %cmd, "no PDU data found in CMGR response");
            return Err(AppError::AtCommand {
                cmd,
                response: "no PDU data in response".into(),
            });
        }

        Ok(RawSmsPdu {
            mem: mem.to_string(),
            index,
            raw_pdu,
        })
    }

    async fn delete_sms(&self, _mem: &str, index: i32) -> Result<(), AppError> {
        let cmd = format!("AT+CMGD={}", index);
        info!(cmd = %cmd, "deleting SMS");

        let lines = self.client.send_command(&cmd).await?;

        if AtClient::is_error(&lines) {
            let cme = AtClient::cme_error(&lines);
            error!(
                cmd = %cmd,
                cme = ?cme,
                "failed to delete SMS"
            );
            return Err(AppError::AtCommand {
                cmd,
                response: format!("CME error: {:?}", cme),
            });
        }

        Ok(())
    }

    async fn query_status(&self) -> Result<ModemStatus, AppError> {
        let mut status = ModemStatus {
            sim_ready: false,
            registered: false,
            roaming: false,
            csq: None,
            rssi_dbm: None,
            operator: None,
            last_error: None,
        };

        // CPIN
        if let Ok(lines) = self.client.send_command("AT+CPIN?").await {
            for l in &lines {
                if let AtLine::Cpin(s) = l {
                    status.sim_ready = s.contains("READY");
                }
            }
        }

        // CSQ
        if let Ok(lines) = self.client.send_command("AT+CSQ").await {
            for l in &lines {
                if let AtLine::Csq(rssi, _ber) = l {
                    status.csq = Some(*rssi);
                    // Convert RSSI to dBm (99 = unknown, 0 = -113 dBm, 1 = -111, ..., 31 = -51)
                    if *rssi < 99 {
                        status.rssi_dbm = Some(-113 + 2 * rssi);
                    }
                }
            }
        }

        // CREG
        if let Ok(lines) = self.client.send_command("AT+CREG?").await {
            for l in &lines {
                if let AtLine::Creg(_n, Some(stat)) = l {
                    status.registered = *stat == 1 || *stat == 5;
                    status.roaming = *stat == 5;
                }
            }
        }

        // COPS
        if let Ok(lines) = self.client.send_command("AT+COPS?").await {
            for l in &lines {
                if let AtLine::Cops(s) = l {
                    // COPS response format: +COPS: <mode>[,<format>,<oper>]
                    // e.g., +COPS: 0,0,"China Mobile"
                    if let Some(start) = s.rfind('"') {
                        if let Some(end) = s[..start].rfind('"') {
                            status.operator = Some(s[end + 1..start].to_string());
                        }
                    }
                }
            }
        }

        Ok(status)
    }
}
