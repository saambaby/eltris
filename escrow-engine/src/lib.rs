//! Non-custodial Lightning escrow backend for task marketplaces
//!
//! This crate implements a trust-minimized Bitcoin/Lightning escrow system using:
//! - Lightning Development Kit (LDK) for hold invoices
//! - Nostr for public auditability
//! - PostgreSQL for state management
//! - Cryptographic verification for security

pub mod engine;
pub mod error;
pub mod models;
pub mod node;
pub mod nostr_publisher;
pub mod payment_coordinator;
pub mod reputation_indexer;
pub mod task_manager;
pub mod verification_service;

use error::EscrowError;

/// Result type alias for escrow operations
pub type EscrowResult<T> = Result<T, EscrowError>;
