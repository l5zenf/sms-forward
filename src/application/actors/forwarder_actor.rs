//! ForwarderActor — 周 N 秒 tick：claim_next_pending → forward → mark_sent/mark_failed.
//! 详见 plain.txt §10 ForwarderActor.

use std::sync::Arc;

use kameo::actor::{Actor, ActorRef};
use kameo::mailbox::unbounded::UnboundedMailbox;
use kameo::message::{Context, Message};
use tracing::{error, info, warn};

use crate::application::actors::messages::ForwardTick;
use crate::domain::port::forwarder_port::ForwarderPort;
use crate::domain::port::sms_repository::SmsRepository;

pub struct ForwarderActor {
    repo: Arc<dyn SmsRepository>,
    forwarder: Arc<dyn ForwarderPort>,
    worker_id: String,
}

impl ForwarderActor {
    pub fn new(
        repo: Arc<dyn SmsRepository>,
        forwarder: Arc<dyn ForwarderPort>,
        worker_id: String,
    ) -> Self {
        Self {
            repo,
            forwarder,
            worker_id,
        }
    }
}

impl Actor for ForwarderActor {
    type Mailbox = UnboundedMailbox<Self>;

    async fn on_start(&mut self, actor_ref: ActorRef<Self>) -> Result<(), kameo::error::BoxError> {
        info!("ForwarderActor starting");
        Ok(())
    }
}

impl Message<ForwardTick> for ForwarderActor {
    type Reply = ();

    async fn handle(&mut self, _msg: ForwardTick, _ctx: Context<'_, Self, Self::Reply>) {
        let claimed = self.repo.claim_next_pending(&self.worker_id).await;
        let sms = match claimed {
            Ok(Some(s)) => s,
            Ok(None) => return,
            Err(e) => {
                error!(error = %e, "[FWD] claim_next_pending failed");
                return;
            }
        };

        info!(
            id = sms.id,
            sender = ?sms.sender,
            content_len = sms.content.as_deref().map(|s| s.chars().count()).unwrap_or(0),
            retry = sms.retry_count,
            "[FWD] claimed SMS, forwarding"
        );
        match self.forwarder.forward(&sms).await {
            Ok(result) if result.success => {
                if let Err(e) = self.repo.mark_sent(sms.id, result.response.clone()).await {
                    error!(id = sms.id, error = %e, "[FWD] mark_sent failed");
                } else {
                    info!(id = sms.id, response_len = result.response.len(), "[FWD] ✓ forwarded + marked sent");
                }
            }
            Ok(result) => {
                warn!(id = sms.id, response = %result.response, "[FWD] forwarder returned non-success");
                if let Err(e) = self.repo.mark_failed(sms.id, result.response).await {
                    error!(id = sms.id, error = %e, "[FWD] mark_failed failed");
                }
            }
            Err(e) => {
                warn!(id = sms.id, error = %e, "[FWD] forward error, will retry");
                if let Err(e2) = self.repo.mark_failed(sms.id, e.to_string()).await {
                    error!(id = sms.id, error = %e2, "[FWD] mark_failed failed");
                }
            }
        }
    }
}
