//! Error types for the escrow system
//!
//! Comprehensive error handling for all escrow operations including
//! LDK integration, database operations, cryptographic verification,
//! and business logic validation.

use thiserror::Error;

/// Main error type for escrow operations
#[derive(Error, Debug)]
pub enum EscrowError {
    /// External integration errors (LDK/Nostr/DB/etc.)
    #[error("Integration error: {0}")]
    Integration(String),

    /// Cryptographic verification errors
    #[error("Cryptographic error: {0}")]
    Crypto(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Payment processing errors
    #[error("Payment error: {0}")]
    Payment(String),

    /// Task validation errors
    #[error("Task validation error: {0}")]
    TaskValidation(String),

    /// Proof verification errors
    #[error("Proof verification error: {0}")]
    ProofVerification(String),

    /// Dispute resolution errors
    #[error("Dispute error: {0}")]
    Dispute(String),

    /// Reputation system errors
    #[error("Reputation error: {0}")]
    Reputation(String),

    /// State machine transition errors
    #[error("Invalid state transition: {from_state} -> {to_state}: {reason}")]
    StateTransition {
        from_state: String,
        to_state: String,
        reason: String,
    },

    /// Invoice errors
    #[error("Invoice error: {0}")]
    Invoice(String),

    /// Timeout errors
    #[error("Timeout error: {0}")]
    Timeout(String),

    /// External API errors
    #[error("External API error: {0}")]
    ExternalApi(String),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// UUID parsing errors
    #[error("UUID parsing error: {0}")]
    Uuid(#[from] uuid::Error),

    /// General internal errors
    #[error("Internal error: {0}")]
    Internal(String),
}

impl EscrowError {
    /// Create a cryptographic error
    pub fn crypto<S: Into<String>>(msg: S) -> Self {
        Self::Crypto(msg.into())
    }

    /// Create a configuration error
    pub fn config<S: Into<String>>(msg: S) -> Self {
        Self::Config(msg.into())
    }

    /// Create a payment error
    pub fn payment<S: Into<String>>(msg: S) -> Self {
        Self::Payment(msg.into())
    }

    /// Create a task validation error
    pub fn task_validation<S: Into<String>>(msg: S) -> Self {
        Self::TaskValidation(msg.into())
    }

    /// Create a proof verification error
    pub fn proof_verification<S: Into<String>>(msg: S) -> Self {
        Self::ProofVerification(msg.into())
    }

    /// Create a dispute error
    pub fn dispute<S: Into<String>>(msg: S) -> Self {
        Self::Dispute(msg.into())
    }

    /// Create a reputation error
    pub fn reputation<S: Into<String>>(msg: S) -> Self {
        Self::Reputation(msg.into())
    }

    /// Create a state transition error
    pub fn state_transition<S: Into<String>>(from_state: S, to_state: S, reason: S) -> Self {
        Self::StateTransition {
            from_state: from_state.into(),
            to_state: to_state.into(),
            reason: reason.into(),
        }
    }

    /// Create an invoice error
    pub fn invoice<S: Into<String>>(msg: S) -> Self {
        Self::Invoice(msg.into())
    }

    /// Create a timeout error
    pub fn timeout<S: Into<String>>(msg: S) -> Self {
        Self::Timeout(msg.into())
    }

    /// Create an external API error
    pub fn external_api<S: Into<String>>(msg: S) -> Self {
        Self::ExternalApi(msg.into())
    }

    /// Create an integration error
    pub fn integration<S: Into<String>>(msg: S) -> Self {
        Self::Integration(msg.into())
    }

    /// Create an internal error
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        Self::Internal(msg.into())
    }
}
