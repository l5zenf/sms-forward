//! ReaperActor — 每 60s tick：recover_stale_sending 把卡住的 sending 恢复成 pending.
//! 详见 plain.txt §10 ReaperActor.

use std::sync::Arc;

use kameo::actor::Actor;
use kameo::mailbox::unbounded::UnboundedMailbox;
use kameo::message::{Context, Message};
use tracing::{error, info};

use crate::application::actors::messages::RecoverStaleSending;
use crate::domain::port::sms_repository::SmsRepository;

pub struct ReaperActor {
    repo: Arc<dyn SmsRepository>,
}

impl ReaperActor {
    pub fn new(repo: Arc<dyn SmsRepository>) -> Self {
        Self { repo }
    }
}

impl Actor for ReaperActor {
    type Mailbox = UnboundedMailbox<Self>;
}

impl Message<RecoverStaleSending> for ReaperActor {
    type Reply = ();

    async fn handle(&mut self, _msg: RecoverStaleSending, _ctx: Context<'_, Self, Self::Reply>) {
        match self.repo.recover_stale_sending().await {
            Ok(count) if count > 0 => {
                info!(recovered = count, "ReaperActor recovered stale sending messages");
            }
            Ok(_) => {}
            Err(e) => error!(error = %e, "ReaperActor recover_stale_sending failed"),
        }
    }
}
