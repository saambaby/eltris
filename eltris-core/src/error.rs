use thiserror::Error;

#[derive(Error, Debug)]
pub enum EltrisError {
    #[error("Exchange error: {0}")]
    Exchange(String),

    #[error("Lightning network error: {0}")]
    Lightning(String),

    #[error("Bitcoin transaction error: {0}")]
    Bitcoin(String),

    #[error("Arbitrage calculation error: {0}")]
    Arbitrage(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: String, available: String },

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, EltrisError>;

impl From<serde_json::Error> for EltrisError {
    fn from(err: serde_json::Error) -> Self {
        EltrisError::Serialization(err.to_string())
    }
}
