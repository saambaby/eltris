//! Core data models for the escrow system
//!
//! This module contains all the database models, state machines,
//! and type definitions for the escrow system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::EscrowResult;

/// Task state machine enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    /// Task created but not yet funded
    Draft,
    /// Funding invoice created, awaiting payment
    PendingFunding,
    /// Payment received and held in escrow
    Funded,
    /// Worker has claimed the task
    Claimed,
    /// Proof submitted and approved
    Verified,
    /// Funds released to worker
    Paid,
    /// Funds returned to employer
    Refunded,
    /// Under arbitration
    Disputed,
    /// Deadline passed without completion
    Expired,
}

impl TaskState {
    /// Check if this is a terminal state (no further transitions possible)
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Paid | Self::Refunded | Self::Expired)
    }

    /// Check if this state allows funding
    pub fn can_fund(&self) -> bool {
        matches!(self, Self::Draft)
    }

    /// Check if this state allows claiming
    pub fn can_claim(&self) -> bool {
        matches!(self, Self::Funded)
    }

    /// Check if this state allows proof submission
    pub fn can_submit_proof(&self) -> bool {
        matches!(self, Self::Claimed)
    }

    /// Check if this state allows verification
    pub fn can_verify(&self) -> bool {
        matches!(self, Self::Claimed)
    }

    /// Check if this state allows settlement
    pub fn can_settle(&self) -> bool {
        matches!(self, Self::Verified)
    }

    /// Check if this state allows disputes
    pub fn can_dispute(&self) -> bool {
        matches!(self, Self::Claimed | Self::Verified)
    }
}

/// Funding mode enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FundingMode {
    /// Lightning hold invoice (preferred)
    LightningHold,
    /// Standard Lightning invoice with manual verification
    LightningStandard,
    /// Boltz submarine swap (on-chain → Lightning)
    OnchainSubmarine,
    /// Boltz reverse swap (Lightning → on-chain)
    OnchainReverse,
    /// Multi-signature on-chain escrow (last resort)
    OnchainMultisig,
}

/// Funding status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FundingStatus {
    /// Funding method created, awaiting payment
    Created,
    /// Payment detected but unconfirmed
    Pending,
    /// Payment confirmed and held
    Accepted,
    /// Preimage revealed, funds released
    Settled,
    /// Hold cancelled, funds returned
    Cancelled,
    /// Expired without payment
    Expired,
    /// Payment or swap failed
    Failed,
}

impl FundingStatus {
    /// Check if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Settled | Self::Cancelled | Self::Expired | Self::Failed)
    }
}

/// Dispute resolution enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisputeResolution {
    /// Awaiting arbitrator review
    Pending,
    /// Funds returned to employer
    EmployerFavor,
    /// Funds released to worker
    WorkerFavor,
    /// Funds split between parties
    Split,
    /// Requires multi-arbitrator review
    Escalated,
    /// Dispute withdrawn by initiator
    Withdrawn,
}

/// Task model representing a marketplace task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub reward_sats: i64,
    pub currency: String,
    pub state: TaskState,

    // Parties
    pub employer_pubkey: String,
    pub worker_pubkey: Option<String>,

    // Funding reference
    pub funding_id: Option<Uuid>,

    // Proof
    pub proof_url: Option<String>,
    pub proof_hash: Option<String>,
    pub proof_nostr_event_id: Option<String>,

    // Verification
    pub verified_by: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub verification_reason: Option<String>,

    // Metadata
    pub deadline: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
    pub nostr_event_id: Option<String>,

    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub settled_at: Option<DateTime<Utc>>,
}

/// Funding model representing payment funding for a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Funding {
    pub id: Uuid,
    pub task_id: Uuid,

    // Payment rail
    pub mode: FundingMode,
    pub provider: String,

    // Lightning (hold invoice)
    pub invoice: Option<String>,
    pub invoice_hash: Option<String>,
    pub preimage_hash: Option<String>,
    pub hold_invoice_id: Option<String>,

    // Amount & expiry
    pub amount_sats: i64,
    pub expires_at: Option<DateTime<Utc>>,

    // On-chain / Submarine swap
    pub onchain_address: Option<String>,
    pub swap_id: Option<String>,
    pub lockup_script: Option<String>,
    pub timeout_block: Option<i32>,

    // Status tracking
    pub status: FundingStatus,
    pub payment_received_at: Option<DateTime<Utc>>,
    pub settled_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,

    // External references
    pub external_id: Option<String>,
    pub external_metadata: Option<serde_json::Value>,

    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Escrow event for audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscrowEvent {
    pub id: i64,
    pub event_type: String,

    // References
    pub task_id: Option<Uuid>,
    pub funding_id: Option<Uuid>,

    // Event data
    pub invoice_hash: Option<String>,
    pub preimage: Option<String>,
    pub amount_sats: Option<i64>,

    // Actor
    pub actor_pubkey: Option<String>,

    // Metadata
    pub provider: Option<String>,
    pub status: Option<String>,
    pub metadata: Option<serde_json::Value>,

    // Cryptographic proof
    pub nostr_event_id: Option<String>,
    pub signature: Option<String>,

    // Timestamp (immutable)
    pub created_at: DateTime<Utc>,
}

/// Reputation model for user scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reputation {
    pub pubkey: String,

    // Scores (0-1000)
    pub score: i32,
    pub tier: String,

    // Stats as employer
    pub tasks_created: i32,
    pub tasks_funded: i32,
    pub tasks_cancelled: i32,
    pub total_sats_paid: i64,

    // Stats as worker
    pub tasks_claimed: i32,
    pub tasks_completed: i32,
    pub tasks_failed: i32,
    pub total_sats_earned: i64,

    // Quality metrics
    pub disputes_total: i32,
    pub disputes_won: i32,
    pub disputes_lost: i32,
    pub avg_completion_time_hours: Option<f64>,
    pub avg_rating: Option<f64>,

    // Badges
    pub badges: Vec<String>,

    // Penalties
    pub penalty_points: i32,
    pub suspended_until: Option<DateTime<Utc>>,

    // Timestamps
    pub first_seen_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Dispute model for arbitration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispute {
    pub id: Uuid,
    pub task_id: Uuid,

    // Parties
    pub initiated_by: String,
    pub respondent: String,

    // Reason
    pub reason: String,
    pub evidence_urls: Vec<String>,

    // Arbitration
    pub arbitrator_pubkey: Option<String>,
    pub resolution: Option<DisputeResolution>,
    pub resolution_reason: Option<String>,

    // Outcome
    pub winner: Option<String>,
    pub funds_distribution: Option<serde_json::Value>,

    // Reputation impact
    pub penalty_employer: i32,
    pub penalty_worker: i32,

    // Timestamps
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,

    // Nostr reference
    pub nostr_event_id: Option<String>,
}

/// User model for caching Nostr profile data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub pubkey: String,

    // Nostr profile (NIP-05)
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub nip05: Option<String>,
    pub nip05_verified: bool,

    // Contact
    pub lud16: Option<String>, // Lightning address
    pub lud06: Option<String>, // LNURL

    // Settings
    pub settings: serde_json::Value,

    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

/// Hold invoice data from LDK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldInvoiceData {
    pub invoice: String,
    pub invoice_hash: String,
    pub hold_invoice_id: String,
    pub amount_sats: u64,
    pub expires_at: DateTime<Utc>,
}

/// Invoice settlement data from LDK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceSettlementData {
    pub invoice_hash: String,
    pub preimage: String,
    pub amount_sats: u64,
    pub settled_at: DateTime<Utc>,
}

/// State transition validation
#[derive(Debug, Clone)]
pub struct StateTransition {
    pub from_state: TaskState,
    pub to_state: TaskState,
    pub reason: String,
    pub valid: bool,
}

impl Task {
    /// Create a new task
    pub fn new(
        title: String,
        description: Option<String>,
        reward_sats: i64,
        employer_pubkey: String,
        deadline: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            title,
            description,
            reward_sats,
            currency: "BTC".to_string(),
            state: TaskState::Draft,
            employer_pubkey,
            worker_pubkey: None,
            funding_id: None,
            proof_url: None,
            proof_hash: None,
            proof_nostr_event_id: None,
            verified_by: None,
            verified_at: None,
            verification_reason: None,
            deadline,
            metadata: None,
            nostr_event_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            claimed_at: None,
            completed_at: None,
            settled_at: None,
        }
    }

    /// Validate a state transition
    pub fn validate_transition(&self, to_state: TaskState) -> EscrowResult<StateTransition> {
        use crate::EscrowError;

        let transition = StateTransition {
            from_state: self.state,
            to_state,
            reason: String::new(),
            valid: false,
        };

        // Define valid transitions
        let valid = match (&self.state, &to_state) {
            (TaskState::Draft, TaskState::PendingFunding) => true,
            (TaskState::Draft, TaskState::Expired) => true,
            (TaskState::PendingFunding, TaskState::Funded) => true,
            (TaskState::PendingFunding, TaskState::Expired) => true,
            (TaskState::PendingFunding, TaskState::Draft) => true,
            (TaskState::Funded, TaskState::Claimed) => true,
            (TaskState::Funded, TaskState::Refunded) => true,
            (TaskState::Funded, TaskState::Expired) => true,
            (TaskState::Claimed, TaskState::Verified) => true,
            (TaskState::Claimed, TaskState::Disputed) => true,
            (TaskState::Claimed, TaskState::Expired) => true,
            (TaskState::Verified, TaskState::Paid) => true,
            (TaskState::Verified, TaskState::Disputed) => true,
            (TaskState::Disputed, TaskState::Paid) => true,
            (TaskState::Disputed, TaskState::Refunded) => true,
            // Split outcome handled via disputes; no direct TaskState::Split
            _ => false,
        };

        if valid {
            Ok(StateTransition {
                valid: true,
                ..transition
            })
        } else {
            Err(EscrowError::state_transition(
                format!("{:?}", self.state),
                format!("{:?}", to_state),
                "Invalid state transition".to_string(),
            ))
        }
    }
}

impl Funding {
    /// Create new funding for a task
    pub fn new(
        task_id: Uuid,
        mode: FundingMode,
        provider: String,
        amount_sats: i64,
        expires_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            task_id,
            mode,
            provider,
            invoice: None,
            invoice_hash: None,
            preimage_hash: None,
            hold_invoice_id: None,
            amount_sats,
            expires_at,
            onchain_address: None,
            swap_id: None,
            lockup_script: None,
            timeout_block: None,
            status: FundingStatus::Created,
            payment_received_at: None,
            settled_at: None,
            cancelled_at: None,
            external_id: None,
            external_metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl Reputation {
    /// Create new reputation entry
    pub fn new(pubkey: String) -> Self {
        Self {
            pubkey,
            score: 500, // Start at intermediate level
            tier: "New".to_string(),
            tasks_created: 0,
            tasks_funded: 0,
            tasks_cancelled: 0,
            total_sats_paid: 0,
            tasks_claimed: 0,
            tasks_completed: 0,
            tasks_failed: 0,
            total_sats_earned: 0,
            disputes_total: 0,
            disputes_won: 0,
            disputes_lost: 0,
            avg_completion_time_hours: None,
            avg_rating: None,
            badges: Vec::new(),
            penalty_points: 0,
            suspended_until: None,
            first_seen_at: Utc::now(),
            last_active_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Calculate tier based on score
    pub fn calculate_tier(&mut self) {
        self.tier = match self.score {
            0..=99 => "New".to_string(),
            100..=299 => "Beginner".to_string(),
            300..=599 => "Intermediate".to_string(),
            600..=799 => "Advanced".to_string(),
            800..=949 => "Trusted".to_string(),
            950..=1000 => "Elite".to_string(),
            _ => "New".to_string(),
        };
    }

    /// Update score based on task completion
    pub fn update_score(&mut self, completed: bool, amount_sats: i64, on_time: bool) {
        let base_points = if completed { 50 } else { -25 };
        let amount_bonus = (amount_sats / 10000).min(50) as i32; // Up to 50 bonus points for large amounts
        let time_bonus = if on_time { 20 } else { -10 };

        self.score = (self.score + base_points + amount_bonus + time_bonus).max(0).min(1000);
        self.calculate_tier();
    }
}

impl Dispute {
    /// Create new dispute
    pub fn new(
        task_id: Uuid,
        initiated_by: String,
        respondent: String,
        reason: String,
        evidence_urls: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            task_id,
            initiated_by,
            respondent,
            reason,
            evidence_urls,
            arbitrator_pubkey: None,
            resolution: Some(DisputeResolution::Pending),
            resolution_reason: None,
            winner: None,
            funds_distribution: None,
            penalty_employer: 0,
            penalty_worker: 0,
            created_at: Utc::now(),
            resolved_at: None,
            nostr_event_id: None,
        }
    }
}
