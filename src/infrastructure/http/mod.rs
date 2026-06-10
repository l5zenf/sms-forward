//! axum HTTP adapter: exposes read-only `/api/*` query endpoints and serves
//! the built React SPA from `web/dist` (with an index.html fallback so client
//! routing works on refresh).
//!
//! This is the interface layer in the DDD sense: it knows about HTTP concerns
//! only and delegates all data access to the [SmsRepository] port, keeping the
//! domain layer framework-agnostic.

use std::path::PathBuf;
use std::sync::Arc;

use axum::{response::Redirect, routing::any, Router};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tracing::{error, info};

use crate::domain::port::sms_repository::SmsRepository;

pub mod error;
pub mod routes;
pub mod state;

pub use state::AppState;

/// Configuration for the HTTP layer, parsed from the `[api]` config section.
#[derive(Debug, Clone)]
pub struct HttpOptions {
    /// Bind address, e.g. `0.0.0.0:8080`.
    pub addr: String,
    /// Allowed CORS origins. An empty list means "reflect the request origin".
    /// Each entry should be a full origin, e.g. `http://localhost:5173`.
    pub cors_origins: Vec<String>,
    /// Root directory of the built frontend, e.g. `web/dist`. If it does not
    /// exist the API still runs; visiting `/` just 404s instead of the SPA.
    pub web_dir: PathBuf,
}

/// Build the full router: `/api/*` JSON endpoints layered under CORS, plus a
/// `ServeDir` static handler for the SPA at `/`.
pub fn build_router(repo: Arc<dyn SmsRepository>, opts: &HttpOptions) -> Router {
    let cors = build_cors(&opts.cors_origins);

    // API sub-tree. `nest` keeps `/api/*` matching before the SPA fallback.
    let api = routes::api_routes();

    let mut app = Router::new()
        .nest("/api", api)
        .layer(cors)
        .with_state(AppState::new(repo));

    // Serve the built SPA if present. Unknown non-/api paths fall back to
    // index.html so client-side routing (deep links) works on a hard refresh.
    // The fallback only fires when no registered route (incl. /api) matches.
    let index = opts.web_dir.join("index.html");
    if opts.web_dir.is_dir() && index.exists() {
        let spa = ServeDir::new(&opts.web_dir).fallback(ServeFile::new(index));
        app = app.fallback_service(spa);
        info!(web_dir = %opts.web_dir.display(), "web UI mounted at /");
    } else {
        info!("web/dist not found; serving API only");
        // Friendly redirect from `/` to the API so root isn't a bare 404.
        app = app.route("/", any(|| async { Redirect::temporary("/api/health") }));
    }

    app
}

fn build_cors(origins: &[String]) -> CorsLayer {
    // An explicit allow-list keeps production tight; an empty list relaxes to
    // `Any` so a developer hitting the API from an arbitrary origin (Vite dev
    // server) isn't blocked. The Vite dev proxy already forwards same-origin,
    // but direct browser calls during development use the explicit origin.
    if origins.is_empty() {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        let allowed = origins
            .iter()
            .filter_map(|o| o.trim().parse().ok())
            .collect::<Vec<_>>();
        CorsLayer::new()
            .allow_origin(allowed)
            .allow_methods(Any)
            .allow_headers(Any)
    }
}

/// Run the HTTP server on the given address until shutdown. Intended to be
/// `tokio::spawn`-ed alongside the actor tick loops in `main`.
pub async fn run(router: Router, addr: &str) {
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            error!(addr = addr, error = %e, "failed to bind HTTP listener");
            return;
        }
    };
    info!(addr = addr, "HTTP server listening");
    if let Err(e) = axum::serve(listener, router).await {
        error!(error = %e, "HTTP server stopped");
    }
}
