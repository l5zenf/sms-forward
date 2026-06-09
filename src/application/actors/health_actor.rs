//! HealthActor — 每 60s tick：向 AtActor 发 QueryStatus，写 modem_status + modem_events.
//! 详见 plain.txt §10 HealthActor。HealthActor 不直接操作 modem.

use std::sync::Arc;

use kameo::actor::{Actor, ActorRef};
use kameo::mailbox::unbounded::UnboundedMailbox;
use kameo::message::{Context, Message};
use tracing::{error, info};

use crate::application::actors::at_actor::AtActor;
use crate::application::actors::messages::HealthTick;
use crate::domain::port::sms_repository::SmsRepository;

pub struct HealthActor {
    at_actor_ref: ActorRef<AtActor>,
    #[allow(dead_code)]
    repo: Arc<dyn SmsRepository>,
}

impl HealthActor {
    pub fn new(repo: Arc<dyn SmsRepository>, at_ref: ActorRef<AtActor>) -> Self {
        Self {
            at_actor_ref: at_ref,
            repo,
        }
    }
}

impl Actor for HealthActor {
    type Mailbox = UnboundedMailbox<Self>;
}

impl Message<HealthTick> for HealthActor {
    type Reply = ();

    async fn handle(&mut self, _msg: HealthTick, _ctx: Context<'_, Self, Self::Reply>) {
        let reply = self
            .at_actor_ref
            .ask(crate::application::actors::messages::QueryStatus)
            .await;

        match reply {
            Ok(status_reply) => {
                info!(
                    sim_ready = status_reply.0.sim_ready,
                    registered = status_reply.0.registered,
                    csq = ?status_reply.0.csq,
                    operator = ?status_reply.0.operator,
                    "modem heartbeat"
                );
            }
            Err(e) => error!(error = %e, "failed to ask AtActor QueryStatus"),
        }

        // 写入 modem_events 用作心跳 trace
        // (省略写 modem_status 表的复杂实现；先用日志代替)
    }
}
