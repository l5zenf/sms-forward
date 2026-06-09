//! AtActor — 独占串口、监听 URC、收到 +CMTI 后读 PDU 发送给 SmsIngestActor，
//! 收到 SmsPersisted 后删除模块短信。详见 plain.txt §10。

use std::sync::Arc;

use kameo::actor::{Actor, ActorRef};
use kameo::mailbox::unbounded::UnboundedMailbox;
use kameo::message::{Context, Message};
use kameo::Reply;
use tracing::{error, info, warn};

use crate::application::actors::messages::{ModemStatusReply, QueryStatus, RawSmsPduReceived};
use crate::application::actors::sms_ingest_actor::SmsIngestActor;
use crate::domain::error::AppError;
use crate::domain::model::ModemStatus;
use crate::domain::port::modem_port::{ModemEvent, ModemPort};

pub struct AtActor {
    modem: Arc<dyn ModemPort>,
    sms_ingest_ref: ActorRef<SmsIngestActor>,
}

impl AtActor {
    pub fn new(modem: Arc<dyn ModemPort>, sms_ingest_ref: ActorRef<SmsIngestActor>) -> Self {
        Self {
            modem,
            sms_ingest_ref,
        }
    }
}

impl Actor for AtActor {
    type Mailbox = UnboundedMailbox<Self>;

    async fn on_start(&mut self, actor_ref: ActorRef<Self>) -> Result<(), kameo::error::BoxError> {
        info!("AtActor starting, initializing modem...");

        if let Err(e) = self.modem.init().await {
            error!(error = %e, "modem init failed");
            return Err(Box::new(e));
        }

        let modem = self.modem.clone();
        let sms_ingest = self.sms_ingest_ref.clone();
        let weak = actor_ref.downgrade();
        tokio::spawn(async move {
            urc_poll_loop(modem, sms_ingest, weak).await;
        });

        info!("AtActor started");
        Ok(())
    }
}

async fn urc_poll_loop(
    modem: Arc<dyn ModemPort>,
    sms_ingest: ActorRef<SmsIngestActor>,
    _self_ref: kameo::actor::WeakActorRef<AtActor>,
) {
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_URC_SECS: AtomicI64 = AtomicI64::new(0);
    let mut idle_pings: u32 = 0;
    let boot_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    LAST_URC_SECS.store(boot_secs, Ordering::Relaxed);

    fn now_secs() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    loop {
        match modem.next_event().await {
            Ok(ModemEvent::NewSms(raw)) => {
                LAST_URC_SECS.store(now_secs(), Ordering::Relaxed);
                idle_pings = 0;
                info!(
                    mem = %raw.mem,
                    index = raw.index,
                    pdu_len = raw.raw_pdu.len(),
                    "[AT] NewSms URC received, dispatching to ingest"
                );
                let msg = RawSmsPduReceived {
                    mem: raw.mem.clone(),
                    index: raw.index,
                    raw_pdu: raw.raw_pdu,
                };
                if let Err(e) = sms_ingest.tell(msg).await {
                    error!(error = %e, "[AT] failed to tell SmsIngestActor RawSmsPduReceived");
                }
            }
            Ok(ModemEvent::Ring) => info!("[AT] RING (incoming call, MVP-1 ignored)"),
            Ok(ModemEvent::Clip(d)) => info!(clip = %d, "[AT] CLIP (incoming call, MVP-1 ignored)"),
            Ok(ModemEvent::Ready) => info!("[AT] modem ready"),
            Ok(ModemEvent::Error(e)) => warn!(error = %e, "[AT] modem error"),
            Err(e) => {
                error!(error = %e, "[AT] modem next_event failed; will retry in 3s");
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        }

        // Liveness heartbeat: every ~30s of complete silence we log once,
        // so a dead reader is immediately visible instead of looking normal.
        idle_pings = idle_pings.wrapping_add(1);
        if idle_pings % 60 == 0 {
            let idle = now_secs() - LAST_URC_SECS.load(Ordering::Relaxed);
            warn!(idle_secs = idle, "[AT] URC reader silent; reading? maybe stuck");
        }
    }
}

#[derive(Debug, Clone, Reply)]
pub struct DeleteAck(pub bool);

impl Message<crate::application::actors::messages::SmsPersisted> for AtActor {
    type Reply = DeleteAck;

    async fn handle(
        &mut self,
        msg: crate::application::actors::messages::SmsPersisted,
        _ctx: Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        info!(mem = %msg.mem, index = msg.index, "[AT] SmsPersisted, deleting from SIM");
        match self.modem.delete_sms(&msg.mem, msg.index).await {
            Ok(()) => {
                info!(mem = %msg.mem, index = msg.index, "[AT] CMGD OK");
                DeleteAck(true)
            }
            Err(AppError::AtCommand { cmd, response }) => {
                warn!(cmd = %cmd, response = %response, "[AT] CMGD failed at command level");
                DeleteAck(false)
            }
            Err(e) => {
                warn!(error = %e, "[AT] CMGD failed");
                DeleteAck(false)
            }
        }
    }
}

impl Message<QueryStatus> for AtActor {
    type Reply = Result<ModemStatusReply, String>;

    async fn handle(
        &mut self,
        _msg: QueryStatus,
        _ctx: Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        match self.modem.query_status().await {
            Ok(status) => Ok(ModemStatusReply(status)),
            Err(e) => Err(e.to_string()),
        }
    }
}
