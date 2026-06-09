use serde::Deserialize;
use tracing::info;
use crate::domain::error::AppError;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub app: AppConfig,
    pub modem: ModemConfig,
    pub database: DatabaseConfig,
    pub forwarder: ForwarderConfig,
    pub retry: RetryConfig,
    pub worker: WorkerConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub name: String,
    pub instance_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModemConfig {
    pub port: String,
    pub baud_rate: u32,
    pub storage: String,
    pub read_buffer_limit: usize,
    pub init_timeout_secs: u64,
    pub sim_ready_wait_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForwarderConfig {
    pub kind: String,
    pub url: String,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RetryConfig {
    pub max_retry: i32,
    pub base_delay_secs: u64,
    pub max_delay_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkerConfig {
    pub forward_interval_secs: u64,
    pub reaper_interval_secs: u64,
    pub health_interval_secs: u64,
    pub sending_timeout_secs: u64,
}

impl Settings {
    pub fn load() -> Result<Self, AppError> {
        // Resolution order:
        //   1. GG_GUARD_CONFIG=<path>
        //   2. ./config.toml in the current working directory
        //   3. /etc/air780e-smsd/config.toml  (system install)
        //   4. environment-variable fallback (GG_GUARD_* / legacy *_*)
        let config_path = std::env::var("GG_GUARD_CONFIG").ok();
        let cwd_config = std::path::Path::new("config.toml");
        let etc_config = std::path::Path::new("/etc/air780e-smsd/config.toml");

        let (resolved, source) = if let Some(p) = config_path {
            (std::path::PathBuf::from(p), "GG_GUARD_CONFIG")
        } else if cwd_config.exists() {
            (cwd_config.to_path_buf(), "cwd:./config.toml")
        } else if etc_config.exists() {
            (etc_config.to_path_buf(), "/etc/air780e-smsd/config.toml")
        } else {
            (std::path::PathBuf::new(), "<env-fallback>")
        };

        if source == "<env-fallback>" {
            return Ok(Settings::from_env());
        }

        let content = std::fs::read_to_string(&resolved).map_err(|e| AppError::Config {
            message: format!("failed to read {}: {}", resolved.display(), e),
        })?;
        let mut settings: Settings = toml::from_str(&content).map_err(|e| AppError::Config {
            message: format!("failed to parse {}: {}", resolved.display(), e),
        })?;

        // Apply env overrides on top of file values (env wins).
        Settings::apply_env_overrides(&mut settings);
        info!(source = source, path = %resolved.display(), "config loaded");
        Ok(settings)
    }

    fn apply_env_overrides(s: &mut Settings) {
        if let Ok(v) = std::env::var("GG_GUARD_MODEM_PORT").or_else(|_| std::env::var("SERIAL_PORT"))
        {
            s.modem.port = v;
        }
        if let Ok(v) = std::env::var("GG_GUARD_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL"))
        {
            s.database.url = v;
        }
        if let Ok(v) = std::env::var("GG_GUARD_WEBHOOK_URL").or_else(|_| std::env::var("WEBHOOK_URL"))
        {
            s.forwarder.url = v;
        }
    }

    fn from_env() -> Self {
        // Apply env overrides from .env / GG_GUARD_* vars
        let port = std::env::var("GG_GUARD_MODEM_PORT")
            .or_else(|_| std::env::var("SERIAL_PORT"))
            .unwrap_or_else(|_| "/dev/ttyUSB2".to_string());

        let db_url = std::env::var("GG_GUARD_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| "sqlite://sms.db?mode=rwc".to_string());

        let webhook_url =
            std::env::var("GG_GUARD_WEBHOOK_URL")
                .or_else(|_| std::env::var("WEBHOOK_URL"))
                .unwrap_or_else(|_| "https://example.com/webhook".to_string());

        Settings {
            app: AppConfig {
                name: std::env::var("GG_GUARD_APP_NAME").unwrap_or_else(|_| "gg-guard".into()),
                instance_id: std::env::var("GG_GUARD_INSTANCE_ID")
                    .unwrap_or_else(|_| "pi-01".into()),
            },
            modem: ModemConfig {
                port,
                baud_rate: 115200,
                storage: "SM".into(),
                read_buffer_limit: 4096,
                init_timeout_secs: 60,
                sim_ready_wait_secs: 60,
            },
            database: DatabaseConfig {
                url: db_url,
                max_connections: 1,
            },
            forwarder: ForwarderConfig {
                kind: "webhook".into(),
                url: webhook_url,
                timeout_secs: 10,
            },
            retry: RetryConfig {
                max_retry: 10,
                base_delay_secs: 60,
                max_delay_secs: 3600,
            },
            worker: WorkerConfig {
                forward_interval_secs: 3,
                reaper_interval_secs: 60,
                health_interval_secs: 60,
                sending_timeout_secs: 300,
            },
        }
    }
}
