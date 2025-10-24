//! Reputation Indexer - Manages user reputation scoring system
//!
//! This module tracks user reputation based on task completion history,
//! dispute outcomes, and other behavioral factors. It provides reputation
//! scores that influence task creation, claiming, and settlement policies.

use crate::{
    error::EscrowError,
    models::{Reputation, TaskState},
    EscrowResult,
};
use chrono::{Utc};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

/// Configuration for the reputation indexer
#[derive(Debug, Clone)]
pub struct ReputationIndexerConfig {
    /// Initial reputation score for new users
    pub initial_score: i32,
    /// Maximum reputation score
    pub max_score: i32,
    /// Minimum reputation score
    pub min_score: i32,
    /// Score decay factor per month of inactivity
    pub decay_factor: f64,
    /// Enable automatic score updates
    pub enable_auto_updates: bool,
}

impl Default for ReputationIndexerConfig {
    fn default() -> Self {
        Self {
            initial_score: 500,
            max_score: 1000,
            min_score: 0,
            decay_factor: 0.05, // 5% decay per month
            enable_auto_updates: true,
        }
    }
}

/// Main reputation indexer
pub struct ReputationIndexer {
    config: ReputationIndexerConfig,
    /// In-memory reputation storage (in production, this would be a database)
    reputations: Arc<RwLock<HashMap<String, Reputation>>>,
}

/// Reputation update operation
pub type ReputationUpdateFn = Box<dyn Fn(&mut Reputation) + Send + Sync>;

impl ReputationIndexer {
    /// Create a new reputation indexer
    pub fn new(config: ReputationIndexerConfig) -> Self {
        Self {
            config,
            reputations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get reputation for a user
    pub async fn get_reputation(&self, pubkey: &str) -> EscrowResult<Reputation> {
        // Check if user exists, create if not
        if !self.reputations.read().await.contains_key(pubkey) {
            self.create_reputation(pubkey.to_string()).await?;
        }

        // Apply decay for inactive users
        let mut reputations = self.reputations.write().await;
        let reputation = reputations.get_mut(pubkey).unwrap();

        self.apply_decay(reputation);
        reputation.last_active_at = Utc::now();

        Ok(reputation.clone())
    }

    /// Update reputation with a custom function
    pub async fn update_reputation<F>(&self, pubkey: &str, update_fn: F) -> EscrowResult<Reputation>
    where
        F: FnOnce(&mut Reputation) + Send,
    {
        // Ensure user exists
        if !self.reputations.read().await.contains_key(pubkey) {
            self.create_reputation(pubkey.to_string()).await?;
        }

        // Apply update
        let mut reputations = self.reputations.write().await;
        let reputation = reputations.get_mut(pubkey).unwrap();

        update_fn(reputation);
        reputation.updated_at = Utc::now();

        // Ensure score bounds
        reputation.score = reputation.score.max(self.config.min_score).min(self.config.max_score);
        reputation.calculate_tier();

        Ok(reputation.clone())
    }

    /// Update reputation based on task completion
    pub async fn update_for_task_completion(
        &self,
        pubkey: &str,
        task_state: TaskState,
        reward_sats: i64,
        completed_on_time: bool,
    ) -> EscrowResult<Reputation> {
        let base_points = match task_state {
            TaskState::Paid => 50,      // Successfully completed and paid
            TaskState::Refunded => -25, // Task refunded (employer cancelled)
            TaskState::Disputed => -10,  // Task went to dispute
            TaskState::Expired => -5,   // Task expired without completion
            _ => 0,                     // No change for other states
        };

        let amount_bonus = (reward_sats / 10000).min(50) as i32; // Up to 50 bonus points for large amounts
        let time_bonus = if completed_on_time { 20 } else { -10 };

        let total_points = base_points + amount_bonus + time_bonus;

        let min_score = self.config.min_score;
        self.update_reputation(pubkey, move |rep| {
            rep.score = (rep.score + total_points).max(self.config.min_score).min(self.config.max_score);
            rep.last_active_at = Utc::now();
        })
        .await
    }

    /// Update reputation for task creation
    pub async fn update_for_task_creation(
        &self,
        pubkey: &str,
        reward_sats: i64,
    ) -> EscrowResult<Reputation> {
        let creation_bonus = (reward_sats / 50000).min(25) as i32; // Up to 25 bonus points for large tasks

        let min_score = self.config.min_score;
        self.update_reputation(pubkey, move |rep| {
            rep.tasks_created += 1;
            rep.score = (rep.score + creation_bonus).max(self.config.min_score).min(self.config.max_score);
            rep.last_active_at = Utc::now();
        })
        .await
    }

    /// Update reputation for dispute resolution
    pub async fn update_for_dispute_resolution(
        &self,
        employer_pubkey: &str,
        worker_pubkey: &str,
        employer_won: bool,
        penalty_points: i32,
    ) -> EscrowResult<(Reputation, Reputation)> {
        let employer_update = move |rep: &mut Reputation| {
            rep.disputes_total += 1;
            if employer_won {
                rep.disputes_won += 1;
            } else {
                rep.disputes_lost += 1;
                rep.penalty_points += penalty_points;
            }
            rep.last_active_at = Utc::now();
        };

        let worker_update = move |rep: &mut Reputation| {
            rep.disputes_total += 1;
            if !employer_won {
                rep.disputes_won += 1;
            } else {
                rep.disputes_lost += 1;
                rep.penalty_points += penalty_points;
            }
            rep.last_active_at = Utc::now();
        };

        let employer_rep = self.update_reputation(employer_pubkey, employer_update).await?;
        let worker_rep = self.update_reputation(worker_pubkey, worker_update).await?;

        Ok((employer_rep, worker_rep))
    }

    /// Get top users by reputation score
    pub async fn get_top_users(&self, limit: usize) -> EscrowResult<Vec<Reputation>> {
        let mut reputations: Vec<_> = self.reputations.read().await.values().cloned().collect();
        reputations.sort_by(|a, b| b.score.cmp(&a.score));
        reputations.truncate(limit);

        Ok(reputations)
    }

    /// Get users by tier
    pub async fn get_users_by_tier(&self, tier: &str) -> EscrowResult<Vec<Reputation>> {
        let reputations: Vec<_> = self.reputations
            .read()
            .await
            .values()
            .filter(|rep| rep.tier == tier)
            .cloned()
            .collect();

        Ok(reputations)
    }

    /// Apply reputation decay for inactive users
    pub async fn apply_reputation_decay(&self) -> EscrowResult<usize> {
        let mut count = 0;
        let mut reputations = self.reputations.write().await;

        for reputation in reputations.values_mut() {
            if self.should_apply_decay(reputation) {
                self.apply_decay(reputation);
                count += 1;
            }
        }

        Ok(count)
    }

    /// Create initial reputation for a new user
    async fn create_reputation(&self, pubkey: String) -> EscrowResult<()> {
        let reputation = Reputation::new(pubkey.clone());
        self.reputations.write().await.insert(pubkey, reputation);
        Ok(())
    }

    /// Check if decay should be applied to a user
    fn should_apply_decay(&self, reputation: &Reputation) -> bool {
        let months_inactive = (Utc::now() - reputation.last_active_at).num_days() / 30;
        months_inactive > 1 // Apply decay after 1 month of inactivity
    }

    /// Apply decay to reputation score
    fn apply_decay(&self, reputation: &mut Reputation) {
        let months_inactive = (Utc::now() - reputation.last_active_at).num_days() / 30;
        let decay_amount = (reputation.score as f64 * self.config.decay_factor * months_inactive as f64) as i32;
        reputation.score = (reputation.score - decay_amount).max(self.config.min_score);
        reputation.calculate_tier();
    }

    /// Calculate reputation statistics
    pub async fn get_reputation_stats(&self) -> EscrowResult<ReputationStats> {
        let reputations = self.reputations.read().await;
        let values: Vec<&Reputation> = reputations.values().collect();

        if values.is_empty() {
            return Ok(ReputationStats::default());
        }

        let total_users = values.len();
        let avg_score = values.iter().map(|r| r.score).sum::<i32>() / total_users as i32;

        let mut tier_counts = HashMap::new();
        for rep in values {
            *tier_counts.entry(rep.tier.clone()).or_insert(0) += 1;
        }

        Ok(ReputationStats {
            total_users,
            avg_score,
            tier_distribution: tier_counts,
        })
    }

    /// Check if user meets minimum reputation requirements
    pub async fn check_reputation_requirement(
        &self,
        pubkey: &str,
        min_score: i32,
    ) -> EscrowResult<bool> {
        let reputation = self.get_reputation(pubkey).await?;
        Ok(reputation.score >= min_score)
    }

    /// Get reputation tier for a user
    pub async fn get_user_tier(&self, pubkey: &str) -> EscrowResult<String> {
        let reputation = self.get_reputation(pubkey).await?;
        Ok(reputation.tier)
    }

    /// Award badges based on achievements
    pub async fn award_badge(&self, pubkey: &str, badge: String) -> EscrowResult<Reputation> {
        self.update_reputation(pubkey, |rep| {
            if !rep.badges.contains(&badge) {
                rep.badges.push(badge);
            }
        })
        .await
    }

    /// Apply penalty for bad behavior
    pub async fn apply_penalty(&self, pubkey: &str, penalty_points: i32, reason: &str) -> EscrowResult<Reputation> {
        self.update_reputation(pubkey, move |rep| {
            rep.penalty_points += penalty_points;
            rep.score = (rep.score - penalty_points).max(self.config.min_score);

            // Check if suspension is needed
            if rep.penalty_points >= 100 {
                rep.suspended_until = Some(Utc::now() + chrono::Duration::days(7));
            }

            rep.calculate_tier();
        })
        .await
    }

    /// Check if user is suspended
    pub async fn is_user_suspended(&self, pubkey: &str) -> EscrowResult<bool> {
        let reputation = self.get_reputation(pubkey).await?;
        if let Some(suspended_until) = reputation.suspended_until {
            Ok(Utc::now() < suspended_until)
        } else {
            Ok(false)
        }
    }
}

/// Reputation statistics
#[derive(Debug, Clone, Default)]
pub struct ReputationStats {
    pub total_users: usize,
    pub avg_score: i32,
    pub tier_distribution: HashMap<String, usize>,
}

impl Default for ReputationIndexer {
    fn default() -> Self {
        Self::new(ReputationIndexerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_reputation() {
        let indexer = ReputationIndexer::default();
        let reputation = indexer.get_reputation("test_pubkey").await.unwrap();

        assert_eq!(reputation.pubkey, "test_pubkey");
        assert_eq!(reputation.score, 500);
        assert_eq!(reputation.tier, "New");
    }

    #[tokio::test]
    async fn test_update_reputation() {
        let indexer = ReputationIndexer::default();

        let updated = indexer.update_reputation("test_pubkey", |rep| {
            rep.score += 100;
        }).await.unwrap();

        assert_eq!(updated.score, 600);
        assert_eq!(updated.tier, "Intermediate");
    }

    #[tokio::test]
    async fn test_task_completion_update() {
        let indexer = ReputationIndexer::default();

        let reputation = indexer.update_for_task_completion("test_pubkey", TaskState::Paid, 50000, true).await.unwrap();

        assert!(reputation.score > 500); // Should have increased
        assert_eq!(reputation.tasks_completed, 1);
    }
}
