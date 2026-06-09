use std::sync::Arc;

use kameo::actor::PreparedActor;
use sea_orm::Database;
use tracing::{info, warn};

use gg_guard::application::actors::at_actor::AtActor;
use gg_guard::application::actors::forwarder_actor::ForwarderActor;
use gg_guard::application::actors::health_actor::HealthActor;
use gg_guard::application::actors::messages::{ForwardTick, HealthTick, RecoverStaleSending};
use gg_guard::application::actors::reaper_actor::ReaperActor;
use gg_guard::application::actors::sms_ingest_actor::SmsIngestActor;
use gg_guard::config::settings::Settings;
use gg_guard::domain::port::forwarder_port::ForwarderPort;
use gg_guard::domain::port::modem_port::ModemPort;
use gg_guard::domain::port::sms_repository::SmsRepository;
use gg_guard::infrastructure::forwarder::webhook_forwarder::WebhookForwarder;
use gg_guard::infrastructure::modem::air780e_at_modem::Air780eAtModem;
use gg_guard::infrastructure::persistence::migration;
use gg_guard::infrastructure::persistence::seaorm_sms_repository::SeaOrmSmsRepository;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("gg-guard starting...");
    let settings = Settings::load()?;
    info!(
        app = %settings.app.name,
        instance = %settings.app.instance_id,
        port = %settings.modem.port,
        webhook = %settings.forwarder.url,
        "config loaded"
    );

    // ── Infrastructure ────────────────────────────────────────────────
    info!(url = %settings.database.url, "opening database");
    let db = Database::connect(&settings.database.url).await?;
    migration::run(&db).await?;
    info!("migration done");

    let worker_id = settings.app.instance_id.clone();
    let repo: Arc<dyn SmsRepository> =
        Arc::new(SeaOrmSmsRepository::new(db, worker_id.clone()));

    let forwarder: Arc<dyn ForwarderPort> = Arc::new(WebhookForwarder::new(
        settings.forwarder.url.clone(),
        settings.forwarder.timeout_secs,
    ));

    let modem: Arc<dyn ModemPort> = Arc::new(Air780eAtModem::new(
        settings.modem.port.clone(),
        settings.modem.baud_rate,
        settings.modem.read_buffer_limit,
    ));

    // ── Actors: two-phase wiring via PreparedActor ────────────────────
    // Forwarder / Reaper have no peer refs, so plain spawn is enough.
    // At / SmsIngest / Health form a cycle (At↔Ingest, Health→At), so we
    // prepare all three first, then inject the peer refs and finally spawn.
    info!("spawning actors...");

    let reaper_ref = kameo::spawn(ReaperActor::new(repo.clone()));
    let forwarder_ref = kameo::spawn(ForwarderActor::new(
        repo.clone(),
        forwarder.clone(),
        worker_id.clone(),
    ));

    let prepared_at = PreparedActor::<AtActor>::new();
    let prepared_ingest = PreparedActor::<SmsIngestActor>::new();
    let prepared_health = PreparedActor::<HealthActor>::new();

    // Grab refs BEFORE spawn — PreparedActor gives us the ref upfront.
    let at_ref = prepared_at.actor_ref().clone();
    let ingest_ref = prepared_ingest.actor_ref().clone();
    let health_ref = prepared_health.actor_ref().clone();

    // Inject peer refs and spawn. Order between these three doesn't matter
    // because the actor message loop only starts after on_start runs, and
    // on_start for AtActor will init the modem — by then all three are queued.
    prepared_at.spawn(AtActor::new(modem, ingest_ref.clone()));
    prepared_ingest.spawn(SmsIngestActor::new(repo.clone(), at_ref.clone()));
    prepared_health.spawn(HealthActor::new(repo.clone(), at_ref.clone()));

    info!("all actors spawned");

    // ── Tick loops ───────────────────────────────────────────────────
    let forwarder_interval = settings.worker.forward_interval_secs;
    let reaper_interval = settings.worker.reaper_interval_secs;
    let health_interval = settings.worker.health_interval_secs;

    let f_ref = forwarder_ref.clone();
    tokio::spawn(async move {
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_secs(forwarder_interval));
        loop {
            ticker.tick().await;
            let _ = f_ref.tell(ForwardTick).await;
        }
    });

    let r_ref = reaper_ref.clone();
    tokio::spawn(async move {
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_secs(reaper_interval));
        loop {
            ticker.tick().await;
            let _ = r_ref.tell(RecoverStaleSending).await;
        }
    });

    let h_ref = health_ref.clone();
    tokio::spawn(async move {
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_secs(health_interval));
        loop {
            ticker.tick().await;
            let _ = h_ref.tell(HealthTick).await;
        }
    });

    info!("gg-guard running; press Ctrl+C to stop");

    tokio::signal::ctrl_c().await?;
    info!("received Ctrl+C, shutting down...");
    // Best-effort shutdown: stop each actor independently, ignore
    // ActorNotRunning for actors that already exited (e.g. AtActor that
    // bailed during init on a hardware/config problem).
    for (name, res) in [
        ("forwarder", forwarder_ref.stop_gracefully().await),
        ("reaper", reaper_ref.stop_gracefully().await),
        ("health", health_ref.stop_gracefully().await),
        ("at", at_ref.stop_gracefully().await),
        ("ingest", ingest_ref.stop_gracefully().await),
    ] {
        match res {
            Ok(()) => info!(actor = name, "stopped"),
            Err(e) => warn!(actor = name, error = %e, "stop_gracefully skipped"),
        }
    }
    info!("all actors stopped");
    Ok(())
}
