# Data Models

## Database Schema

### Tasks Table

```sql
CREATE TABLE tasks (
  id VARCHAR(64) PRIMARY KEY,
  title VARCHAR(255) NOT NULL,
  description TEXT,
  reward_sats BIGINT NOT NULL,
  currency VARCHAR(10) DEFAULT 'BTC',
  state VARCHAR(50) NOT NULL,
  
  -- Parties
  employer_pubkey VARCHAR(64) NOT NULL,
  worker_pubkey VARCHAR(64),
  
  -- Funding reference
  funding_id VARCHAR(64),
  
  -- Proof
  proof_url TEXT,
  proof_hash VARCHAR(64),
  proof_nostr_event_id VARCHAR(64),
  
  -- Verification
  verified_by VARCHAR(64),
  verified_at TIMESTAMP,
  verification_reason TEXT,
  
  -- Metadata
  deadline TIMESTAMP,
  metadata JSONB,
  nostr_event_id VARCHAR(64),
  
  -- Timestamps
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW(),
  claimed_at TIMESTAMP,
  completed_at TIMESTAMP,
  settled_at TIMESTAMP,
  
  -- Indexes
  INDEX idx_employer (employer_pubkey),
  INDEX idx_worker (worker_pubkey),
  INDEX idx_state (state),
  INDEX idx_created_at (created_at DESC)
);
```

**States:**
- `Draft`: Task created, not yet funded
- `PendingFunding`: Funding invoice created, awaiting payment
- `Funded`: Payment received and held in escrow
- `Claimed`: Worker has claimed the task
- `Verified`: Proof submitted and approved
- `Paid`: Funds released to worker
- `Refunded`: Funds returned to employer
- `Disputed`: Under arbitration
- `Expired`: Deadline passed without completion

---

### Funding Table

```sql
CREATE TABLE funding (
  id VARCHAR(64) PRIMARY KEY,
  task_id VARCHAR(64) NOT NULL REFERENCES tasks(id),
  
  -- Payment rail
  mode VARCHAR(50) NOT NULL,
  provider VARCHAR(50) NOT NULL,
  
  -- Lightning (hold invoice)
  invoice TEXT,
  invoice_hash VARCHAR(64),
  preimage_hash VARCHAR(64),
  hold_invoice_id VARCHAR(64),
  
  -- Amount & expiry
  amount_sats BIGINT NOT NULL,
  expires_at TIMESTAMP,
  
  -- On-chain / Submarine swap
  onchain_address VARCHAR(100),
  swap_id VARCHAR(64),
  lockup_script TEXT,
  timeout_block INTEGER,
  
  -- Status tracking
  status VARCHAR(50) NOT NULL,
  payment_received_at TIMESTAMP,
  settled_at TIMESTAMP,
  cancelled_at TIMESTAMP,
  
  -- External references
  external_id VARCHAR(255),
  external_metadata JSONB,
  
  -- Timestamps
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW(),
  
  -- Indexes
  INDEX idx_task_id (task_id),
  INDEX idx_invoice_hash (invoice_hash),
  INDEX idx_status (status),
  INDEX idx_swap_id (swap_id)
);
```

**Modes:**
- `lightning_hold`: LDK hold invoice (preferred)
- `lightning_standard`: Standard invoice with manual verification
- `onchain_submarine`: Boltz submarine swap (on-chain → Lightning)
- `onchain_reverse`: Boltz reverse swap (Lightning → on-chain)
- `onchain_multisig`: Multi-signature on-chain escrow (last resort)

**Providers:**
- `ldk`: Internal Lightning Development Kit node
- `boltz`: Boltz swap service
- `manual`: Manual verification (testing only)

**Statuses:**
- `created`: Funding method created, awaiting payment
- `pending`: Payment detected but unconfirmed
- `accepted`: Payment confirmed and held
- `settled`: Preimage revealed, funds released
- `cancelled`: Hold cancelled, funds returned
- `expired`: Expired without payment
- `failed`: Payment or swap failed

---

### Escrow Events Table

Immutable append-only log for audit trail.

```sql
CREATE TABLE escrow_events (
  id BIGSERIAL PRIMARY KEY,
  event_type VARCHAR(50) NOT NULL,
  
  -- References
  task_id VARCHAR(64),
  funding_id VARCHAR(64),
  
  -- Event data
  invoice_hash VARCHAR(64),
  preimage VARCHAR(64),
  amount_sats BIGINT,
  
  -- Actor
  actor_pubkey VARCHAR(64),
  
  -- Metadata
  provider VARCHAR(50),
  status VARCHAR(50),
  metadata JSONB,
  
  -- Cryptographic proof
  nostr_event_id VARCHAR(64),
  signature TEXT,
  
  -- Timestamp (immutable)
  created_at TIMESTAMP DEFAULT NOW(),
  
  -- Indexes
  INDEX idx_task_id (task_id),
  INDEX idx_funding_id (funding_id),
  INDEX idx_event_type (event_type),
  INDEX idx_created_at (created_at DESC)
);
```

**Event Types:**
- `invoice.created`
- `invoice.accepted`
- `invoice.held`
- `invoice.settled`
- `invoice.cancelled`
- `invoice.expired`
- `swap.created`
- `swap.detected`
- `swap.confirmed`
- `swap.settled`
- `swap.failed`
- `proof.submitted`
- `proof.verified`
- `proof.rejected`
- `settlement.initiated`
- `settlement.completed`
- `refund.initiated`
- `refund.completed`

---

### Reputation Table

```sql
CREATE TABLE reputation (
  pubkey VARCHAR(64) PRIMARY KEY,
  
  -- Scores (0-1000)
  score INTEGER DEFAULT 500,
  tier VARCHAR(50) DEFAULT 'New',
  
  -- Stats as employer
  tasks_created INTEGER DEFAULT 0,
  tasks_funded INTEGER DEFAULT 0,
  tasks_cancelled INTEGER DEFAULT 0,
  total_sats_paid BIGINT DEFAULT 0,
  
  -- Stats as worker
  tasks_claimed INTEGER DEFAULT 0,
  tasks_completed INTEGER DEFAULT 0,
  tasks_failed INTEGER DEFAULT 0,
  total_sats_earned BIGINT DEFAULT 0,
  
  -- Quality metrics
  disputes_total INTEGER DEFAULT 0,
  disputes_won INTEGER DEFAULT 0,
  disputes_lost INTEGER DEFAULT 0,
  avg_completion_time_hours NUMERIC(10,2),
  avg_rating NUMERIC(3,2),
  
  -- Badges
  badges JSONB DEFAULT '[]',
  
  -- Penalties
  penalty_points INTEGER DEFAULT 0,
  suspended_until TIMESTAMP,
  
  -- Timestamps
  first_seen_at TIMESTAMP DEFAULT NOW(),
  last_active_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW(),
  
  -- Indexes
  INDEX idx_score (score DESC),
  INDEX idx_tier (tier)
);
```

**Tiers:**
- `New`: 0-99 score (default for new users)
- `Beginner`: 100-299 score
- `Intermediate`: 300-599 score
- `Advanced`: 600-799 score
- `Trusted`: 800-949 score
- `Elite`: 950-1000 score

**Badges (examples):**
- `early_adopter`: First 1000 users
- `fast_delivery`: 90% of tasks completed within deadline
- `quality_work`: Average rating > 4.5
- `high_volume`: 100+ tasks completed
- `zero_disputes`: No disputes lost
- `lightning_native`: All payments via Lightning

---

### Disputes Table

```sql
CREATE TABLE disputes (
  id VARCHAR(64) PRIMARY KEY,
  task_id VARCHAR(64) NOT NULL REFERENCES tasks(id),
  
  -- Parties
  initiated_by VARCHAR(64) NOT NULL,
  respondent VARCHAR(64) NOT NULL,
  
  -- Reason
  reason TEXT NOT NULL,
  evidence_urls TEXT[],
  
  -- Arbitration
  arbitrator_pubkey VARCHAR(64),
  resolution VARCHAR(50),
  resolution_reason TEXT,
  
  -- Outcome
  winner VARCHAR(64),
  funds_distribution JSONB,
  
  -- Reputation impact
  penalty_employer INTEGER DEFAULT 0,
  penalty_worker INTEGER DEFAULT 0,
  
  -- Timestamps
  created_at TIMESTAMP DEFAULT NOW(),
  resolved_at TIMESTAMP,
  
  -- Nostr reference
  nostr_event_id VARCHAR(64),
  
  -- Indexes
  INDEX idx_task_id (task_id),
  INDEX idx_status (resolution),
  INDEX idx_arbitrator (arbitrator_pubkey)
);
```

**Resolutions:**
- `pending`: Awaiting arbitrator review
- `employer_favor`: Funds returned to employer
- `worker_favor`: Funds released to worker
- `split`: Funds split between parties
- `escalated`: Requires multi-arbitrator review
- `withdrawn`: Dispute withdrawn by initiator

---

### Users Table

Optional - for caching Nostr profile data.

```sql
CREATE TABLE users (
  pubkey VARCHAR(64) PRIMARY KEY,
  
  -- Nostr profile (NIP-05)
  name VARCHAR(255),
  display_name VARCHAR(255),
  about TEXT,
  picture TEXT,
  nip05 VARCHAR(255),
  nip05_verified BOOLEAN DEFAULT FALSE,
  
  -- Contact
  lud16 VARCHAR(255), -- Lightning address
  lud06 TEXT,         -- LNURL
  
  -- Settings
  settings JSONB DEFAULT '{}',
  
  -- Timestamps
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW(),
  last_seen_at TIMESTAMP
);
```

---

## Rust Type Hints

While this document doesn't include Rust code, here are suggested Rust crate mappings for the schema:

### Recommended Crates

```toml
[dependencies]
# Database
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-native-tls", "macros", "bigdecimal", "chrono"] }
diesel = { version = "2.1", features = ["postgres", "chrono"] }  # Alternative to sqlx

# Time handling
chrono = "0.4"

# JSON handling
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# GraphQL
async-graphql = "7.0"
async-graphql-axum = "7.0"  # If using Axum

# Cryptography
secp256k1 = "0.28"
sha2 = "0.10"
hex = "0.4"

# Lightning/Bitcoin
lightning = "0.0.118"  # LDK
bitcoin = "0.31"
```

### Example Rust Type Mapping

```rust
// State enums would use Rust enums with serde:

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Draft,
    PendingFunding,
    Funded,
    Claimed,
    Verified,
    Paid,
    Refunded,
    Disputed,
    Expired,
}

// Database models would use sqlx::FromRow or diesel::Queryable:

#[derive(Debug, FromRow)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub reward_sats: i64,
    // ... other fields
    pub created_at: DateTime<Utc>,
}

// GraphQL types would use async-graphql::Object:

#[derive(async_graphql::SimpleObject)]
pub struct TaskGraphQL {
    pub id: ID,
    pub title: String,
    pub reward_sats: String, // BigInt as String
    // ... other fields
}
```

### Database Constraints Implementation

PostgreSQL constraints should be enforced at database level (shown in SQL above) and validated in Rust application layer using pattern matching and type system.

---

## State Transition Rules

### Task State Machine

```
Draft 
  ├─> PendingFunding (when funding invoice created)
  └─> Expired (if deadline passed)

PendingFunding
  ├─> Funded (when payment accepted)
  ├─> Expired (if invoice expires)
  └─> Draft (if cancelled before payment)

Funded
  ├─> Claimed (when worker claims)
  ├─> Refunded (if employer cancels before claim)
  └─> Expired (if deadline passed with no claim)

Claimed
  ├─> Verified (when proof approved)
  ├─> Disputed (if proof rejected)
  └─> Expired (if deadline passed without proof)

Verified
  ├─> Paid (when settlement completes)
  └─> Disputed (if employer disputes after verification)

Disputed
  ├─> Paid (if arbitrator rules for worker)
  ├─> Refunded (if arbitrator rules for employer)
  └─> Paid (split settlement if partial favor)

Paid, Refunded, Expired = Terminal states
```

### Funding Status Machine

```
created
  ├─> pending (payment detected)
  ├─> expired (timeout reached)
  └─> cancelled (manual cancellation)

pending
  ├─> accepted (payment confirmed & held)
  └─> failed (payment routing failed)

accepted
  ├─> settled (preimage revealed)
  ├─> cancelled (hold released, refund)
  └─> expired (hold timeout)

settled, cancelled, expired, failed = Terminal states
```
