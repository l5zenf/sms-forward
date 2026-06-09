use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum AppError {
    #[snafu(display("config error: {message}"))]
    Config { message: String },

    #[snafu(display("serial open failed: path={path}, error={detail}"))]
    SerialOpen {
        path: String,
        detail: String,
    },

    #[snafu(display("AT command failed: cmd={cmd}, response={response}"))]
    AtCommand { cmd: String, response: String },

    #[snafu(display("AT command timeout: cmd={cmd}"))]
    AtTimeout { cmd: String },

    #[snafu(display("modem init failed: step={step}"))]
    ModemInit { step: String },

    #[snafu(display("PDU decode failed: reason={reason}, raw={raw_pdu}"))]
    PduDecode { reason: String, raw_pdu: String },

    #[snafu(display("database operation failed: op={op}"))]
    Database {
        op: String,
        source: sea_orm::DbErr,
    },

    #[snafu(display("forward failed: target={target}"))]
    Forward {
        target: String,
        source: reqwest::Error,
    },

    #[snafu(display("invalid state transition: from={from}, to={to}"))]
    InvalidState { from: String, to: String },

    #[snafu(display("actor send failed: actor={actor}"))]
    ActorSend { actor: String },

    #[snafu(display("serial I/O error: {message}"))]
    SerialIo { message: String },

    #[snafu(display("modem not initialized"))]
    ModemNotInitialized,
}
