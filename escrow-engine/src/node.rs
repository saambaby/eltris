//! Main Escrow Node - High-level API for the escrow system
//!
//! This module provides the main interface for interacting with the escrow system.
//! It coordinates all the underlying components (LDK, TaskManager, etc.) and provides
//! a clean API for task lifecycle management.

use crate::{
    EscrowResult,
    engine::{EscrowEngine, EscrowEngineConfig, LiquidityInfo, NodeInfo},
    error::EscrowError,
    models::{Dispute, EscrowEvent, Funding, FundingMode, Reputation, Task, TaskState, User},
    nostr_publisher::{NostrPublisher, NostrPublisherConfig},
    payment_coordinator::{PaymentCoordinator, PaymentCoordinatorConfig},
    reputation_indexer::{ReputationIndexer, ReputationIndexerConfig},
    task_manager::{TaskManager, TaskManagerConfig},
    verification_service::{VerificationService, VerificationServiceConfig},
};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// Configuration for the escrow node
#[derive(Debug, Clone)]
pub struct EscrowNodeConfig {
    /// Task manager configuration
    pub task_config: TaskManagerConfig,
    /// Escrow engine configuration
    pub escrow_config: EscrowEngineConfig,
    /// Payment coordinator configuration
    pub payment_config: PaymentCoordinatorConfig,
    /// Verification service configuration
    pub verification_config: VerificationServiceConfig,
    /// Nostr publisher configuration
    pub nostr_config: NostrPublisherConfig,
    /// Reputation indexer configuration
    pub reputation_config: ReputationIndexerConfig,
}

impl Default for EscrowNodeConfig {
    fn default() -> Self {
        Self {
            task_config: TaskManagerConfig::default(),
            escrow_config: EscrowEngineConfig::default(),
            payment_config: PaymentCoordinatorConfig::default(),
            verification_config: VerificationServiceConfig::default(),
            nostr_config: NostrPublisherConfig::default(),
            reputation_config: ReputationIndexerConfig::default(),
        }
    }
}

/// Main escrow node that coordinates all components
pub struct EscrowNode {
    /// Task manager for task lifecycle
    task_manager: Arc<TaskManager>,
    /// Escrow engine for LDK integration
    escrow_engine: Arc<EscrowEngine>,
    /// Payment coordinator for payment rails
    payment_coordinator: Arc<PaymentCoordinator>,
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

/// Task information response
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub task: Task,
    pub funding: Option<Funding>,
    pub events: Vec<EscrowEvent>,
    pub reputation: Option<Reputation>,
}

/// User tasks response
#[derive(Debug, Clone)]
pub struct UserTasksResponse {
    pub tasks: Vec<Task>,
    pub total_count: usize,
}

/// Reputation statistics response
#[derive(Debug, Clone)]
pub struct ReputationStatsResponse {
    pub stats: crate::reputation_indexer::ReputationStats,
    pub user_reputation: Option<Reputation>,
}

impl EscrowNode {
    /// Create a new escrow node with all components initialized
    pub async fn new(config: EscrowNodeConfig) -> EscrowResult<Self> {
        info!("Initializing escrow node with all components");

        // Initialize escrow engine (LDK)
        let escrow_engine: Arc<EscrowEngine> =
            Arc::new(EscrowEngine::new(config.escrow_config).await?);
        let verification_service: Arc<VerificationService> =
            Arc::new(VerificationService::new(config.verification_config));
        let nostr_publisher: Arc<NostrPublisher> =
            Arc::new(NostrPublisher::new(config.nostr_config).await?);
        let reputation_indexer: Arc<ReputationIndexer> =
            Arc::new(ReputationIndexer::new(config.reputation_config));
        let payment_coordinator: Arc<PaymentCoordinator> =
            Arc::new(PaymentCoordinator::new(config.payment_config));

        // Initialize task manager
        let task_manager = Arc::new(
            TaskManager::new(
                config.task_config,
                escrow_engine.clone(),
                verification_service.clone(),
                nostr_publisher.clone(),
                reputation_indexer.clone(),
            )
            .await?,
        );

        info!("Escrow node initialized successfully");

        Ok(Self {
            task_manager,
            escrow_engine,
            payment_coordinator,
            verification_service,
            nostr_publisher,
            reputation_indexer,
        })
    }

    /// Create a new task
    pub async fn create_task(&self, request: CreateTaskRequest) -> EscrowResult<Task> {
        let task_request = crate::task_manager::CreateTaskRequest {
            title: request.title,
            description: request.description,
            reward_sats: request.reward_sats,
            employer_pubkey: request.employer_pubkey,
            deadline: request.deadline,
            metadata: request.metadata,
        };

        self.task_manager.create_task(task_request).await
    }

    /// Fund a task with a hold invoice
    pub async fn fund_task(
        &self,
        request: FundTaskRequest,
    ) -> EscrowResult<crate::models::HoldInvoiceData> {
        let fund_request = crate::task_manager::FundTaskRequest {
            task_id: request.task_id,
            employer_pubkey: request.employer_pubkey,
            mode: request.mode,
        };

        self.task_manager.fund_task(fund_request).await
    }

    /// Submit proof of work completion
    pub async fn submit_proof(&self, request: SubmitProofRequest) -> EscrowResult<Task> {
        let submit_proof_request = crate::task_manager::SubmitProofRequest {
            task_id: request.task_id,
            worker_pubkey: request.worker_pubkey,
            proof_url: request.proof_url,
            proof_hash: request.proof_hash,
            nostr_event_id: request.nostr_event_id,
            nostr_signature: request.nostr_signature,
        };
        self.task_manager.submit_proof(submit_proof_request).await
    }

    /// Verify task completion and approve for payment
    pub async fn verify_task(&self, request: VerifyTaskRequest) -> EscrowResult<Task> {
        let verify_request = crate::task_manager::VerifyTaskRequest {
            task_id: request.task_id,
            verifier_pubkey: request.verifier_pubkey,
            approved: request.approved,
            reason: request.reason,
            signature: request.signature,
        };

        self.task_manager.verify_task(verify_request).await
    }

    /// Get task information with related data
    pub async fn get_task_info(&self, task_id: Uuid) -> EscrowResult<TaskInfo> {
        let task = self.task_manager.get_task(task_id).await?;
        let events = self.task_manager.get_task_events(task_id).await?;

        let funding = if let Some(funding_id) = task.funding_id {
            Some(self.task_manager.get_funding(funding_id).await?)
        } else {
            None
        };

        // Get employer reputation
        let reputation = self
            .reputation_indexer
            .get_reputation(&task.employer_pubkey)
            .await
            .ok();

        Ok(TaskInfo {
            task,
            funding,
            events,
            reputation,
        })
    }

    /// Get all tasks for a user (as employer or worker)
    pub async fn get_user_tasks(&self, pubkey: &str) -> EscrowResult<UserTasksResponse> {
        let tasks = self.task_manager.get_user_tasks(pubkey).await?;

        Ok(UserTasksResponse {
            total_count: tasks.len(),
            tasks,
        })
    }

    /// Get reputation information for a user
    pub async fn get_user_reputation(&self, pubkey: &str) -> EscrowResult<Reputation> {
        self.reputation_indexer.get_reputation(pubkey).await
    }

    /// Get reputation statistics for the system
    pub async fn get_reputation_stats(&self) -> EscrowResult<ReputationStatsResponse> {
        let stats = self.reputation_indexer.get_reputation_stats().await?;

        Ok(ReputationStatsResponse {
            stats,
            user_reputation: None, // Would need user context to fill this
        })
    }

    /// Get supported payment modes for an amount
    pub fn get_supported_payment_modes(&self, amount_sats: u64) -> Vec<FundingMode> {
        self.payment_coordinator.get_supported_modes(amount_sats)
    }

    /// Calculate estimated fees for a payment
    pub fn calculate_payment_fees(&self, amount_sats: u64, mode: FundingMode) -> u64 {
        self.payment_coordinator.calculate_fees(amount_sats, mode)
    }

    /// Get node liquidity information
    pub async fn get_liquidity_info(&self) -> EscrowResult<LiquidityInfo> {
        self.escrow_engine.get_liquidity_info().await
    }

    /// Get node information
    pub async fn get_node_info(&self) -> EscrowResult<NodeInfo> {
        self.escrow_engine.get_node_info().await
    }

    /// Health check for the escrow node
    pub async fn health_check(&self) -> EscrowResult<NodeHealth> {
        // Check if all components are healthy
        let mut issues = Vec::new();

        // Check LDK node
        if let Err(e) = self.escrow_engine.get_node_info().await {
            issues.push(format!("LDK node error: {}", e));
        }

        // Check reputation indexer
        if let Err(e) = self.reputation_indexer.get_reputation_stats().await {
            issues.push(format!("Reputation indexer error: {}", e));
        }

        // Check Nostr publisher
        if let Err(e) = self.nostr_publisher.get_task_events(Uuid::new_v4()).await {
            issues.push(format!("Nostr publisher error: {}", e));
        }

        Ok(NodeHealth {
            healthy: issues.is_empty(),
            issues,
            timestamp: Utc::now(),
        })
    }

    /// Shutdown the escrow node gracefully
    pub async fn shutdown(&self) -> EscrowResult<()> {
        info!("Shutting down escrow node");

        // In production, this would:
        // 1. Stop LDK node gracefully
        // 2. Close database connections
        // 3. Cancel pending operations
        // 4. Publish shutdown events

        info!("Escrow node shutdown complete");

        Ok(())
    }
}

/// Node health status
#[derive(Debug, Clone)]
pub struct NodeHealth {
    pub healthy: bool,
    pub issues: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

impl Default for EscrowNode {
    fn default() -> Self {
        // This would panic in production - use new() instead
        unimplemented!("Use EscrowNode::new() to create an instance")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_node_initialization() {
        let config = EscrowNodeConfig::default();
        let node = EscrowNode::new(config).await.unwrap();

        let health = node.health_check().await.unwrap();
        assert!(health.healthy);
    }

    #[tokio::test]
    async fn test_task_creation() {
        let config = EscrowNodeConfig::default();
        let node = EscrowNode::new(config).await.unwrap();

        let request = CreateTaskRequest {
            title: "Test Task".to_string(),
            description: Some("Test description".to_string()),
            reward_sats: 50000,
            employer_pubkey: "employer_pubkey".to_string(),
            deadline: None,
            metadata: None,
        };

        let task = node.create_task(request).await.unwrap();
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.reward_sats, 50000);
        assert_eq!(task.state, TaskState::Draft);
    }
}
