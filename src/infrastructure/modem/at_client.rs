use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use serialport::SerialPort;
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{debug, error, info, warn};

use crate::domain::error::AppError;
use crate::infrastructure::modem::at_parser::{self, AtLine};

/// Timeout for AT command responses
const AT_CMD_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum number of URCs queued while no consumer is reading.
const URC_BUFFER: usize = 256;

/// Single-writer / Multiple-reader AT client.
///
/// Architecture
/// ------------
/// One background tokio task owns the serial port and reads lines one at a
/// time. Each parsed line is dispatched:
///
///   * Terminal lines (`Ok` / `Error` / `CmeError`) → complete the most
///     recent pending `send_command` via `oneshot`.
///   * Other lines (`+CMTI`, `+CSQ:`, bare PDU hex, `>`, etc.) are
///     buffered into the pending command's accumulator if a command is in
///     flight AND the line looks like a command reply; otherwise routed to
///     the URC channel for `next_event` to consume.
///
/// Replaces the previous design where `BufReader::new(port)` was recreated
/// on every call and silently consumed URCs that arrived between commands.
pub struct AtClient {
    inner: Arc<Inner>,
    /// Receiver for non-command lines (URCs); polled by `next_event`.
    urc_rx: Mutex<mpsc::UnboundedReceiver<AtLine>>,
}

struct Inner {
    /// Sender into the reader task.
    cmd_tx: mpsc::UnboundedSender<ReaderMsg>,
}

enum ReaderMsg {
    /// Reader-up probe; replies immediately once the port is open.
    OpenProbe(oneshot::Sender<Result<(), String>>),
    /// Atomically: register a pending waiter AND send the command bytes.
    /// Reader registers first, then writes — so any terminal line that
    /// arrives between write and register is still routed to `pending`.
    SendAndRegister {
        cmd: String,
        reply: oneshot::Sender<Vec<AtLine>>,
    },
    /// Cancel any pending wait.
    Cancel,
    /// Send raw bytes (no reply needed).
    Write(String),
}

/// Wraps the port into something Send-safe for the reader task.
struct PortOwner {
    port: StdMutex<Box<dyn SerialPort>>,
}

impl PortOwner {
    /// Blocking read up to and including the next `\n`.
    /// Uses serialport's built-in timeout; returns `Ok(total_bytes)` where
    /// total_bytes=0 means we hit a serial timeout without any input.
    fn read_until_newline(&self, line: &mut String) -> std::io::Result<usize> {
        let mut guard = self.port.lock().map_err(|_| std::io::Error::other("poisoned"))?;
        let mut byte = [0u8; 1];
        let mut total = 0usize;
        // The port has a 100ms poll timeout (we configured during open),
        // so we'll re-enter the caller's loop frequently.
        loop {
            match guard.read(&mut byte) {
                Ok(0) => return Ok(total),
                Ok(_) => {
                    line.push(byte[0] as char);
                    total += 1;
                    if byte[0] == b'\n' {
                        return Ok(total);
                    }
                    if total > 8192 {
                        warn!(len = total, "[AT-reader] line > 8KB, force-flushing");
                        return Ok(total);
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::TimedOut
                        || e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    return Ok(total);
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn write_all(&self, bytes: &[u8]) -> std::io::Result<()> {
        let mut guard = self.port.lock().map_err(|_| std::io::Error::other("poisoned"))?;
        let mut written = 0;
        while written < bytes.len() {
            let n = guard.write(&bytes[written..])?;
            if n == 0 {
                return Err(std::io::Error::other("short write"));
            }
            written += n;
        }
        // tcdrain flush blocks; OS will send within ms at our low baud. Skip.
        Ok(())
    }
}

impl AtClient {
    pub fn new(port_path: String, baud_rate: u32, _buffer_limit: usize) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<ReaderMsg>();
        let (urc_tx, urc_rx) = mpsc::unbounded_channel::<AtLine>();

        let inner = Arc::new(Inner { cmd_tx });

        let client = Self {
            inner: inner.clone(),
            urc_rx: Mutex::new(urc_rx),
        };

        tokio::spawn(reader_task(port_path, baud_rate, cmd_rx, urc_tx));

        client
    }

    /// Wait for the background reader to open the port.
    pub async fn open(&self) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.inner
            .cmd_tx
            .send(ReaderMsg::OpenProbe(tx))
            .map_err(|_| AppError::ModemNotInitialized)?;
        match tokio::time::timeout(Duration::from_secs(5), rx).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(detail))) => Err(AppError::SerialOpen {
                path: "?".into(),
                detail,
            }),
            Ok(Err(_)) => Err(AppError::ModemNotInitialized),
            Err(_) => Err(AppError::AtTimeout {
                cmd: "OPEN".to_string(),
            }),
        }
    }

    pub async fn close(&self) {
        let _ = self.inner.cmd_tx.send(ReaderMsg::Cancel);
    }

    pub async fn is_open(&self) -> bool {
        // Liveness probe: if the channel is closed the reader task died.
        !self.inner.cmd_tx.is_closed()
    }

    /// Send an AT command and wait for the terminal line.
    /// Returns all non-terminal lines received in between, in order.
    pub async fn send_command(&self, cmd: &str) -> Result<Vec<AtLine>, AppError> {
        let (tx, rx) = oneshot::channel();
        self.inner
            .cmd_tx
            .send(ReaderMsg::SendAndRegister {
                cmd: format!("{}\r\n", cmd),
                reply: tx,
            })
            .map_err(|_| AppError::ModemNotInitialized)?;

        debug!(cmd = %cmd, "AT command sent");

        match tokio::time::timeout(AT_CMD_TIMEOUT, rx).await {
            Ok(Ok(lines)) => Ok(lines),
            Ok(Err(_)) => {
                warn!(cmd = %cmd, "[AT] command waiter dropped without reply");
                Err(AppError::ModemNotInitialized)
            }
            Err(_) => {
                let _ = self.inner.cmd_tx.send(ReaderMsg::Cancel);
                Err(AppError::AtTimeout {
                    cmd: cmd.to_string(),
                })
            }
        }
    }

    /// Poll for the next pending URC. Returns None immediately if empty.
    pub async fn read_line(&self) -> Result<Option<AtLine>, AppError> {
        let mut rx = self.urc_rx.lock().await;
        match rx.try_recv() {
            Ok(line) => {
                debug!(line = ?line, "[AT] dequeue URC");
                Ok(Some(line))
            }
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Err(AppError::ModemNotInitialized)
            }
        }
    }

    pub fn is_ok(lines: &[AtLine]) -> bool {
        lines.iter().any(|l| matches!(l, AtLine::Ok))
    }

    pub fn is_error(lines: &[AtLine]) -> bool {
        lines
            .iter()
            .any(|l| matches!(l, AtLine::Error | AtLine::CmeError(..)))
    }

    pub fn find_data_prefix<'a>(lines: &'a [AtLine], prefix: &str) -> Option<&'a str> {
        lines.iter().find_map(|l| {
            if let AtLine::Data(s) = l {
                if s.starts_with(prefix) {
                    Some(s.as_str())
                } else {
                    None
                }
            } else {
                None
            }
        })
    }

    pub fn cme_error(lines: &[AtLine]) -> Option<(i32, String)> {
        lines.iter().find_map(|l| {
            if let AtLine::CmeError(code, msg) = l {
                Some((*code, msg.clone()))
            } else {
                None
            }
        })
    }
}

/// True if this line is an unsolicited URC that the modem pushes
/// spontaneously (never as a response to a command). Everything else —
/// including `+CPIN:` / `+CSQ:` / `+CREG:` style replies — is treated as
/// a command response when a `send_command` is in flight.
fn is_urc(line: &AtLine) -> bool {
    matches!(
        line,
        AtLine::Cmti { .. } | AtLine::Ring | AtLine::Clip(_)
    )
}

async fn reader_task(
    port_path: String,
    baud_rate: u32,
    mut cmd_rx: mpsc::UnboundedReceiver<ReaderMsg>,
    urc_tx: mpsc::UnboundedSender<AtLine>,
) {
    debug!(port = %port_path, "[AT-reader] task starting");

    let port = match serialport::new(&port_path, baud_rate)
        .timeout(Duration::from_millis(100))
        .data_bits(serialport::DataBits::Eight)
        .stop_bits(serialport::StopBits::One)
        .parity(serialport::Parity::None)
        .flow_control(serialport::FlowControl::None)
        .open()
    {
        Ok(p) => p,
        Err(e) => {
            let detail = e.to_string();
            error!(port = %port_path, error = %detail, "[AT-reader] failed to open serial port");
            while let Some(msg) = cmd_rx.recv().await {
                match msg {
                    ReaderMsg::OpenProbe(s) => { let _ = s.send(Err(detail.clone())); }
                    ReaderMsg::SendAndRegister { reply, .. } => { let _ = reply.send(Vec::new()); }
                    _ => {}
                }
            }
            return;
        }
    };
    info!(port = %port_path, "[AT-reader] serial port opened");

    let port_owner = Arc::new(PortOwner {
        port: StdMutex::new(port),
    });

    // Pending command state. While Some, command-reply lines accumulate into
    // `collected`; the terminal line completes the wait.
    let mut pending: Option<oneshot::Sender<Vec<AtLine>>> = None;
    let mut collected: Vec<AtLine> = Vec::new();

    let mut read_buf = String::new();
    let _start = Instant::now();

    loop {
        // 1) Drain control messages non-blocking.
        while let Ok(msg) = cmd_rx.try_recv() {
            match msg {
                ReaderMsg::OpenProbe(sender) => {
                    let _ = sender.send(Ok(()));
                }
                ReaderMsg::Write(s) => {
                    if let Err(e) = port_owner.write_all(s.as_bytes()) {
                        warn!(error = %e, "[AT-reader] write failed");
                    }
                }
                ReaderMsg::SendAndRegister { cmd, reply } => {
                    // 1a) Resolve any in-flight waiter first.
                    if let Some(old) = pending.take() {
                        let _ = old.send(std::mem::take(&mut collected));
                    }
                    // 1b) Register the new waiter BEFORE the bytes are
                    // written, so any URC/OK that arrives is captured into
                    // `collected` correctly.
                    pending = Some(reply);
                    collected.clear();
                    if let Err(e) = port_owner.write_all(cmd.as_bytes()) {
                        warn!(error = %e, "[AT-reader] write failed");
                    }
                }
                ReaderMsg::Cancel => {
                    if let Some(old) = pending.take() {
                        let _ = old.send(std::mem::take(&mut collected));
                    }
                    collected.clear();
                }
            }
        }

        // 2) Try a non-blocking read of one line. serialport's read timeout
        // returns 0 after 500ms with no data, so we won't truly block.
        read_buf.clear();
        match port_owner.read_until_newline(&mut read_buf) {
            Ok(n) if n > 0 => {
                // Trim CRLF.
                while read_buf.ends_with('\n') || read_buf.ends_with('\r') {
                    read_buf.pop();
                }
                if read_buf.is_empty() {
                    tokio::task::yield_now().await;
                    continue;
                }
                let parsed = at_parser::parse_line(&read_buf);
                debug!(line = %read_buf, parsed = ?parsed, "[AT] raw line");

                let is_terminal = matches!(
                    parsed,
                    AtLine::Ok | AtLine::Error | AtLine::CmeError(..)
                );

                if is_terminal {
                    if let Some(sender) = pending.take() {
                        collected.push(parsed.clone());
                        let _ = sender.send(std::mem::take(&mut collected));
                    } else {
                        urc_push(&urc_tx, parsed);
                    }
                } else if pending.is_some() && !is_urc(&parsed) {
                    // A reply line (e.g. +CPIN: READY, +CSQ: 24,0) for the
                    // command in flight — accumulate so send_command sees it.
                    collected.push(parsed.clone());
                } else {
                    // Either no command in flight, or this is a true URC.
                    urc_push(&urc_tx, parsed);
                }
            }
            Ok(_) => {
                // No data yet; sleep briefly so we don't pin a CPU.
                // 20ms ≈ 50 wakeups/s, latency ≥ priority.
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(e) => {
                warn!(error = %e, "[AT-reader] read error; continuing");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

fn urc_push(tx: &mpsc::UnboundedSender<AtLine>, line: AtLine) {
    if tx.send(line).is_err() {
        warn!("[AT-reader] URC receiver gone; dropping line");
    }
}
