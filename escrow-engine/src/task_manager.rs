//! Task Manager - Coordinates task lifecycle and state transitions
//!
//! This module manages the complete lifecycle of tasks from creation
//! through funding, claiming, verification, and settlement. It coordinates
//! between the EscrowEngine, VerificationService, and other components.

use crate::EscrowResult;
use crate::{
    error::EscrowError,
    escrow_engine::EscrowEngine,
    models::{
        Dispute, EscrowEvent, Funding, FundingMode, FundingStatus, Reputation, Task, TaskState,
        User,
    },
    nostr_publisher::NostrPublisher,
    reputation_indexer::ReputationIndexer,
    verification_service::VerificationService,
};
use chrono::{DateTime, Utc};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Configuration for the task manager
#[derive(Debug, Clone)]
pub struct TaskManagerConfig {
    /// Default task timeout in hours
    pub default_task_timeout_hours: u32,
    /// Maximum task reward in sats
    pub max_task_reward_sats: i64,
    /// Require reputation check for task creation
    pub require_reputation_check: bool,
    /// Minimum reputation score to create tasks
    pub min_reputation_score: i32,
}

impl Default for TaskManagerConfig {
    fn default() -> Self {
        Self {
            default_task_timeout_hours: 168,  // 1 week
            max_task_reward_sats: 10_000_000, // 0.1 BTC
            require_reputation_check: false,
            min_reputation_score: 100,
        }
    }
}

/// Main task manager that coordinates task lifecycle
pub struct TaskManager {
    /// Configuration
    config: TaskManagerConfig,
    /// In-memory task storage (in production, this would be a database)
    tasks: Arc<RwLock<HashMap<Uuid, Task>>>,
    /// In-memory funding storage
    funding: Arc<RwLock<HashMap<Uuid, Funding>>>,
    /// In-memory escrow events storage
    escrow_events: Arc<RwLock<Vec<EscrowEvent>>>,
    /// Escrow engine for LDK integration
    escrow_engine: Arc<EscrowEngine>,
    /// Verification service for proof validation
    verification_service: Arc<VerificationService>,
    /// Nostr publisher for public events
    nostr_publisher: Arc<NostrPublisher>,
    /// Reputation indexer for user scoring
    reputation_indexer: Arc<ReputationIndexer>,
}

/// Task creation request
#[derive(Debug, Clone)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub reward_sats: i64,
    pub employer_pubkey: String,
    pub deadline: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
}

/// Task funding request
#[derive(Debug, Clone)]
pub struct FundTaskRequest {
    pub task_id: Uuid,
    pub employer_pubkey: String,
    pub mode: FundingMode,
}

/// Task claiming request
#[derive(Debug, Clone)]
pub struct ClaimTaskRequest {
    pub task_id: Uuid,
    pub worker_pubkey: String,
    pub worker_invoice: String,
}

/// Proof submission request
#[derive(Debug, Clone)]
pub struct SubmitProofRequest {
    pub task_id: Uuid,
    pub worker_pubkey: String,
    pub proof_url: String,
    pub proof_hash: String,
    pub nostr_event_id: String,
    pub nostr_signature: String,
}

/// Task verification request
#[derive(Debug, Clone)]
pub struct VerifyTaskRequest {
    pub task_id: Uuid,
    pub verifier_pubkey: String,
    pub approved: bool,
    pub reason: String,
    pub signature: String,
}

impl TaskManager {
    /// Create a new task manager
    pub async fn new(
        config: TaskManagerConfig,
        escrow_engine: Arc<EscrowEngine>,
        verification_service: Arc<VerificationService>,
        nostr_publisher: Arc<NostrPublisher>,
        reputation_indexer: Arc<ReputationIndexer>,
    ) -> Result<Self, EscrowError> {
        Ok(Self {
            config,
            tasks: Arc::new(RwLock::new(HashMap::new())),
            funding: Arc::new(RwLock::new(HashMap::new())),
            escrow_events: Arc::new(RwLock::new(Vec::new())),
            escrow_engine,
            verification_service,
            nostr_publisher,
            reputation_indexer,
        })
    }

    /// Create a new task
    pub async fn create_task(&self, request: CreateTaskRequest) -> Result<Task, EscrowError> {
        info!("Creating task: {}", request.title);

        // Validate request
        self.validate_create_task_request(&request)?;

        // Check reputation if required
        if self.config.require_reputation_check {
            let employer_reputation = self
                .reputation_indexer
                .get_reputation(&request.employer_pubkey)
                .await?;

            if employer_reputation.score < self.config.min_reputation_score {
                return Err(EscrowError::reputation(format!(
                    "Insufficient reputation score. Required: {}, Got: {}",
                    self.config.min_reputation_score, employer_reputation.score
                )));
            }
        }

        // Create task
        let mut task = Task::new(
            request.title,
            request.description,
            request.reward_sats,
            request.employer_pubkey,
            request.deadline,
        );

        if let Some(metadata) = request.metadata {
            task.metadata = Some(metadata);
        }

        // Store task
        self.tasks.write().await.insert(task.id, task.clone());

        // Update reputation (task creation)
        self.reputation_indexer
            .update_reputation(&task.employer_pubkey, |rep| {
                rep.tasks_created += 1;
                rep.last_active_at = Utc::now();
            })
            .await?;

        // Publish Nostr event
        self.nostr_publisher
            .publish_task_created(task.clone())
            .await?;

        // Create escrow event
        self.create_escrow_event(
            "task.created".to_string(),
            Some(task.id),
            None,
            None,
            Some(task.employer_pubkey.clone()),
            None,
            Some(serde_json::json!({
                "title": task.title,
                "reward_sats": task.reward_sats
            })),
        )
        .await?;

        info!("Created task: {}", task.id);

        Ok(task)
    }

    /// Fund a task with a hold invoice
    pub async fn fund_task(
        &self,
        request: FundTaskRequest,
    ) -> Result<crate::models::HoldInvoiceData, EscrowError> {
        info!("Funding task: {}", request.task_id);

        // Get task
        let mut task = self.get_task(request.task_id).await?;

        // Validate funding request
        self.validate_fund_task_request(&request, &task)?;

        // Transition task state
        task.validate_transition(TaskState::PendingFunding)?;
        task.state = TaskState::PendingFunding;
        task.updated_at = Utc::now();

        // Create funding record
        let mut funding = Funding::new(
            request.task_id,
            request.mode,
            "ldk".to_string(), // provider
            task.reward_sats,
            Some(Utc::now() + chrono::Duration::hours(1)), // expires in 1 hour
        );

        // Create hold invoice
        let invoice_data = self
            .escrow_engine
            .create_hold_invoice(
                task.reward_sats as u64,
                format!("Task: {}", task.title),
                task.id.to_string(),
            )
            .await?;

        // Update funding record
        funding.invoice = Some(invoice_data.invoice.clone());
        funding.invoice_hash = Some(invoice_data.invoice_hash.clone());
        funding.hold_invoice_id = Some(invoice_data.hold_invoice_id.clone());
        funding.status = FundingStatus::Created;
        funding.expires_at = Some(invoice_data.expires_at);

        // Store funding
        self.funding
            .write()
            .await
            .insert(funding.id, funding.clone());

        // Update task with funding reference
        task.funding_id = Some(funding.id);
        self.tasks.write().await.insert(task.id, task.clone());

        // Create escrow event
        self.create_escrow_event(
            "invoice.created".to_string(),
            Some(request.task_id),
            Some(funding.id),
            Some(invoice_data.invoice_hash.clone()),
            Some(request.employer_pubkey),
            Some(format!("{:?}", funding.status)),
            Some(serde_json::json!({
                "amount_sats": task.reward_sats,
                "invoice": invoice_data.invoice
            })),
        )
        .await?;

        info!(
            "Funded task: {} with invoice: {}",
            request.task_id, invoice_data.invoice_hash
        );

        Ok(invoice_data)
    }

    /// Claim a task for work
    pub async fn claim_task(&self, request: ClaimTaskRequest) -> Result<Task, EscrowError> {
        info!("Claiming task: {}", request.task_id);

        // Get task
        let mut task = self.get_task(request.task_id).await?;

        // Validate claim request
        self.validate_claim_task_request(&request, &task)?;

        // Transition task state
        task.validate_transition(TaskState::Claimed)?;
        task.state = TaskState::Claimed;
        task.worker_pubkey = Some(request.worker_pubkey.clone());
        task.claimed_at = Some(Utc::now());
        task.updated_at = Utc::now();

        // Store updated task
        self.tasks.write().await.insert(task.id, task.clone());

        // Update reputation (task claimed)
        self.reputation_indexer
            .update_reputation(&request.worker_pubkey, |rep| {
                rep.tasks_claimed += 1;
                rep.last_active_at = Utc::now();
            })
            .await?;

        // Publish Nostr event
        self.nostr_publisher
            .publish_task_claimed(task.clone())
            .await?;

        // Create escrow event
        self.create_escrow_event(
            "task.claimed".to_string(),
            Some(request.task_id),
            task.funding_id,
            None,
            Some(request.worker_pubkey),
            None,
            None,
        )
        .await?;

        info!("Claimed task: {}", request.task_id);

        Ok(task)
    }

    /// Submit proof of work completion
    pub async fn submit_proof(&self, request: SubmitProofRequest) -> Result<Task, EscrowError> {
        info!("Submitting proof for task: {}", request.task_id);

        // Get task
        let mut task = self.get_task(request.task_id).await?;

        // Validate proof submission
        self.validate_proof_submission(&request, &task)?;

        // Verify Nostr signature
        self.verification_service
            .verify_nostr_signature(&request.nostr_signature, &request.nostr_event_id)
            .await?;

        // Update task with proof
        task.proof_url = Some(request.proof_url.clone());
        task.proof_hash = Some(request.proof_hash.clone());
        task.proof_nostr_event_id = Some(request.nostr_event_id.clone());
        task.updated_at = Utc::now();

        // Store updated task
        self.tasks.write().await.insert(task.id, task.clone());

        // Create escrow event
        self.create_escrow_event(
            "proof.submitted".to_string(),
            Some(request.task_id),
            task.funding_id,
            None,
            Some(request.worker_pubkey),
            None,
            Some(serde_json::json!({
                "proof_url": request.proof_url,
                "proof_hash": request.proof_hash,
                "nostr_event_id": request.nostr_event_id
            })),
        )
        .await?;

        info!("Submitted proof for task: {}", request.task_id);

        Ok(task)
    }

    /// Verify task completion and approve for payment
    pub async fn verify_task(&self, request: VerifyTaskRequest) -> Result<Task, EscrowError> {
        info!("Verifying task: {}", request.task_id);

        // Get task
        let mut task = self.get_task(request.task_id).await?;

        // Validate verification request
        self.validate_verification_request(&request, &task)?;

        // Verify signature
        self.verification_service
            .verify_signature(&request.signature, &request.verifier_pubkey)
            .await?;

        if request.approved {
            // Approve and transition to verified state
            task.validate_transition(TaskState::Verified)?;
            task.state = TaskState::Verified;
            task.verified_by = Some(request.verifier_pubkey.clone());
            task.verified_at = Some(Utc::now());
            task.verification_reason = Some(request.reason.clone());
            task.completed_at = Some(Utc::now());
            task.updated_at = Utc::now();

            // Proceed to settlement
            self.settle_task(task.id).await?;
        } else {
            // Reject and create dispute
            task.state = TaskState::Disputed;
            task.updated_at = Utc::now();

            // Create dispute
            if let Some(ref worker_pubkey) = task.worker_pubkey {
                let dispute = Dispute::new(
                    task.id,
                    request.verifier_pubkey.clone(),
                    worker_pubkey.clone(),
                    request.reason.clone(),
                    vec![],
                );

                // In production, store dispute in database
                warn!("Created dispute for task: {}", task.id);
            }
        }

        // Store updated task
        self.tasks.write().await.insert(task.id, task.clone());

        // Publish Nostr event
        if request.approved {
            self.nostr_publisher
                .publish_task_verified(task.clone())
                .await?;
        } else {
            self.nostr_publisher
                .publish_task_disputed(task.clone())
                .await?;
        }

        // Create escrow event
        self.create_escrow_event(
            if request.approved {
                "proof.verified".to_string()
            } else {
                "proof.rejected".to_string()
            },
            Some(request.task_id),
            task.funding_id,
            None,
            Some(request.verifier_pubkey),
            None,
            Some(serde_json::json!({
                "approved": request.approved,
                "reason": request.reason
            })),
        )
        .await?;

        info!(
            "Verified task: {} (approved: {})",
            request.task_id, request.approved
        );

        Ok(task)
    }

    /// Settle a verified task by releasing funds
    async fn settle_task(
        &self,
        task_id: Uuid,
    ) -> Result<crate::models::InvoiceSettlementData, EscrowError> {
        info!("Settling task: {}", task_id);

        // Get task and funding
        let task = self.get_task(task_id).await?;
        let funding = self.get_funding(task.funding_id.unwrap()).await?;

        // Get worker invoice from task claim (in production, this would be stored)
        let worker_invoice = "worker_invoice_placeholder".to_string(); // TODO: Get from task data

        // Settle hold invoice
        let settlement_data = self
            .escrow_engine
            .settle_hold_invoice(&funding.hold_invoice_id.unwrap(), &worker_invoice)
            .await?;

        // Update task state
        let mut task = self.get_task(task_id).await?;
        task.state = TaskState::Paid;
        task.settled_at = Some(settlement_data.settled_at);
        task.updated_at = Utc::now();
        self.tasks.write().await.insert(task.id, task.clone());

        // Update funding status
        let mut funding = self.get_funding(funding.id).await?;
        funding.status = FundingStatus::Settled;
        funding.settled_at = Some(settlement_data.settled_at);
        self.funding
            .write()
            .await
            .insert(funding.id, funding.clone());

        // Update reputation scores
        if let Some(ref worker_pubkey) = task.worker_pubkey {
            self.reputation_indexer
                .update_reputation(worker_pubkey, move |rep| {
                    rep.tasks_completed += 1;
                    rep.total_sats_earned += task.reward_sats;
                    rep.update_score(true, task.reward_sats, true); // completed, on time
                    rep.last_active_at = Utc::now();
                })
                .await?;

            let employer_pubkey = task.employer_pubkey.clone();
            let reward_sats = task.reward_sats;
            self.reputation_indexer
                .update_reputation(&employer_pubkey, move |rep| {
                    rep.tasks_funded += 1;
                    rep.total_sats_paid += reward_sats;
                    rep.last_active_at = Utc::now();
                })
                .await?;
        }

        // Publish Nostr event
        self.nostr_publisher.publish_task_paid(task.clone()).await?;

        // Create escrow event
        self.create_escrow_event(
            "settlement.completed".to_string(),
            Some(task_id),
            Some(funding.id),
            Some(settlement_data.invoice_hash.clone()),
            None,
            Some("Settled".to_string()),
            Some(serde_json::json!({
                "amount_sats": task.reward_sats,
                "preimage": settlement_data.preimage
            })),
        )
        .await?;

        info!("Settled task: {}", task_id);

        Ok(settlement_data)
    }

    /// Get a task by ID
    pub async fn get_task(&self, task_id: Uuid) -> Result<Task, EscrowError> {
        self.tasks
            .read()
            .await
            .get(&task_id)
            .cloned()
            .ok_or_else(|| EscrowError::task_validation(format!("Task {} not found", task_id)))
    }

    /// Get funding by ID
    pub async fn get_funding(&self, funding_id: Uuid) -> Result<Funding, EscrowError> {
        self.funding
            .read()
            .await
            .get(&funding_id)
            .cloned()
            .ok_or_else(|| {
                EscrowError::task_validation(format!("Funding {} not found", funding_id))
            })
    }

    /// Get all tasks for a user
    pub async fn get_user_tasks(&self, pubkey: &str) -> Result<Vec<Task>, EscrowError> {
        let tasks = self.tasks.read().await;
        let user_tasks = tasks
            .values()
            .filter(|task| {
                task.employer_pubkey == pubkey
                    || task.worker_pubkey.as_ref() == Some(&pubkey.to_string())
            })
            .cloned()
            .collect();

        Ok(user_tasks)
    }

    /// Get escrow events for a task
    pub async fn get_task_events(&self, task_id: Uuid) -> Result<Vec<EscrowEvent>, EscrowError> {
        let events = self.escrow_events.read().await;
        let task_events = events
            .iter()
            .filter(|event| event.task_id == Some(task_id))
            .cloned()
            .collect();

        Ok(task_events)
    }

    /// Create an escrow event for audit trail
    async fn create_escrow_event(
        &self,
        event_type: String,
        task_id: Option<Uuid>,
        funding_id: Option<Uuid>,
        invoice_hash: Option<String>,
        actor_pubkey: Option<String>,
        status: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<(), EscrowError> {
        let event = EscrowEvent {
            id: 0, // Would be auto-generated by database
            event_type,
            task_id,
            funding_id,
            invoice_hash,
            preimage: None,
            amount_sats: None,
            actor_pubkey,
            provider: None,
            status,
            metadata,
            nostr_event_id: None,
            signature: None,
            created_at: Utc::now(),
        };

        self.escrow_events.write().await.push(event);

        Ok(())
    }

    /// Validate task creation request
    fn validate_create_task_request(&self, request: &CreateTaskRequest) -> Result<(), EscrowError> {
        if request.title.trim().is_empty() {
            return Err(EscrowError::task_validation("Title cannot be empty"));
        }

        if request.reward_sats <= 0 {
            return Err(EscrowError::task_validation(
                "Reward must be greater than 0",
            ));
        }

        if request.reward_sats > self.config.max_task_reward_sats {
            return Err(EscrowError::task_validation(format!(
                "Reward {} sats exceeds maximum {}",
                request.reward_sats, self.config.max_task_reward_sats
            )));
        }

        if request.employer_pubkey.trim().is_empty() {
            return Err(EscrowError::task_validation(
                "Employer pubkey cannot be empty",
            ));
        }

        Ok(())
    }

    /// Validate funding request
    fn validate_fund_task_request(
        &self,
        request: &FundTaskRequest,
        task: &Task,
    ) -> Result<(), EscrowError> {
        if task.employer_pubkey != request.employer_pubkey {
            return Err(EscrowError::task_validation(
                "Only task creator can fund task",
            ));
        }

        if !task.state.can_fund() {
            return Err(EscrowError::state_transition(
                format!("{:?}", task.state),
                "PendingFunding".to_string(),
                "Task cannot be funded in current state".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate claim request
    fn validate_claim_task_request(
        &self,
        request: &ClaimTaskRequest,
        task: &Task,
    ) -> Result<(), EscrowError> {
        if task.state != TaskState::Funded {
            return Err(EscrowError::state_transition(
                format!("{:?}", task.state),
                "Claimed".to_string(),
                "Only funded tasks can be claimed".to_string(),
            ));
        }

        if request.worker_pubkey.trim().is_empty() {
            return Err(EscrowError::task_validation(
                "Worker pubkey cannot be empty",
            ));
        }

        if request.worker_invoice.trim().is_empty() {
            return Err(EscrowError::task_validation(
                "Worker invoice cannot be empty",
            ));
        }

        Ok(())
    }

    /// Validate proof submission
    fn validate_proof_submission(
        &self,
        request: &SubmitProofRequest,
        task: &Task,
    ) -> Result<(), EscrowError> {
        if task.worker_pubkey.as_ref() != Some(&request.worker_pubkey) {
            return Err(EscrowError::task_validation(
                "Only assigned worker can submit proof",
            ));
        }

        if !task.state.can_submit_proof() {
            return Err(EscrowError::state_transition(
                format!("{:?}", task.state),
                "Claimed".to_string(),
                "Proof can only be submitted for claimed tasks".to_string(),
            ));
        }

        if request.proof_url.trim().is_empty() {
            return Err(EscrowError::task_validation("Proof URL cannot be empty"));
        }

        if request.proof_hash.trim().is_empty() {
            return Err(EscrowError::task_validation("Proof hash cannot be empty"));
        }

        Ok(())
    }

    /// Validate verification request
    fn validate_verification_request(
        &self,
        request: &VerifyTaskRequest,
        task: &Task,
    ) -> Result<(), EscrowError> {
        // Only employer or system can verify
        if request.verifier_pubkey != task.employer_pubkey {
            return Err(EscrowError::task_validation("Only task creator can verify"));
        }

        if !task.state.can_verify() {
            return Err(EscrowError::state_transition(
                format!("{:?}", task.state),
                "Verified".to_string(),
                "Only claimed tasks can be verified".to_string(),
            ));
        }

        Ok(())
    }
}
