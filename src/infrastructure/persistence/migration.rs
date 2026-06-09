use sea_orm::{ConnectionTrait, DbBackend, Statement};

/// Run raw SQL migrations. Uses IF NOT EXISTS so it's safe to run on every startup.
pub async fn run(db: &sea_orm::DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    // PRAGMA
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "PRAGMA journal_mode=WAL;",
    ))
    .await?;

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "PRAGMA synchronous=NORMAL;",
    ))
    .await?;

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "PRAGMA busy_timeout=5000;",
    ))
    .await?;

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "PRAGMA foreign_keys=ON;",
    ))
    .await?;

    // sms_messages
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE IF NOT EXISTS sms_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            iccid TEXT,
            sender TEXT,
            content TEXT,
            sms_time TEXT,
            received_at TEXT NOT NULL DEFAULT (datetime('now')),
            pdu_raw TEXT NOT NULL,
            dcs INTEGER,
            encoding TEXT,
            concat_ref TEXT,
            concat_total INTEGER,
            concat_completed INTEGER NOT NULL DEFAULT 1,
            modem_mem TEXT,
            modem_index INTEGER,
            dedupe_key TEXT NOT NULL UNIQUE,
            status TEXT NOT NULL DEFAULT 'pending',
            retry_count INTEGER NOT NULL DEFAULT 0,
            max_retry INTEGER NOT NULL DEFAULT 10,
            next_retry_at TEXT NOT NULL DEFAULT (datetime('now')),
            locked_at TEXT,
            locked_by TEXT,
            forwarded_at TEXT,
            forward_response TEXT,
            last_error TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        "#,
    ))
    .await?;

    // Indexes for sms_messages
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE INDEX IF NOT EXISTS idx_sms_status_retry ON sms_messages(status, next_retry_at, id);",
    ))
    .await?;

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE INDEX IF NOT EXISTS idx_sms_received_at ON sms_messages(received_at);",
    ))
    .await?;

    // sms_parts
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE IF NOT EXISTS sms_parts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            iccid TEXT,
            sender TEXT NOT NULL,
            sms_time TEXT,
            concat_ref TEXT NOT NULL,
            concat_total INTEGER NOT NULL,
            concat_seq INTEGER NOT NULL,
            pdu_raw TEXT NOT NULL,
            decoded_content TEXT NOT NULL,
            dcs INTEGER,
            encoding TEXT,
            modem_mem TEXT,
            modem_index INTEGER,
            part_dedupe_key TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        "#,
    ))
    .await?;

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE INDEX IF NOT EXISTS idx_sms_parts_concat ON sms_parts(sender, concat_ref, concat_total);",
    ))
    .await?;

    // modem_events
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE IF NOT EXISTS modem_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL,
            payload TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        "#,
    ))
    .await?;

    // modem_status
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE IF NOT EXISTS modem_status (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            sim_ready INTEGER NOT NULL DEFAULT 0,
            registered INTEGER NOT NULL DEFAULT 0,
            roaming INTEGER NOT NULL DEFAULT 0,
            csq INTEGER,
            rssi_dbm INTEGER,
            operator TEXT,
            last_error TEXT,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        "#,
    ))
    .await?;

    Ok(())
}
