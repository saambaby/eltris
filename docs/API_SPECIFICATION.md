# GraphQL API Specification

## Endpoint
```
POST https://api.yourdomain.com/graphql
```

## Authentication

All authenticated requests require HTTP headers:

```
X-Nostr-Pubkey: <hex_pubkey>
X-Nostr-Signature: <schnorr_signature>
X-Nostr-Timestamp: <unix_timestamp>
```

Signature is computed over: `graphql:<operation_name>:<timestamp>:<query_hash>`

---

## GraphQL Schema

### Scalar Types

```graphql
scalar DateTime
scalar JSON
scalar BigInt
scalar Hex
```

---

### Enums

```graphql
enum TaskState {
  DRAFT
  PENDING_FUNDING
  FUNDED
  CLAIMED
  VERIFIED
  PAID
  REFUNDED
  DISPUTED
  EXPIRED
}

enum FundingMode {
  LIGHTNING_SELF
  LIGHTNING_ANY
  ONCHAIN_SUBMARINE
  ONCHAIN_REVERSE
  ONCHAIN_MULTISIG
}

enum FundingStatus {
  CREATED
  PENDING
  ACCEPTED
  SETTLED
  CANCELLED
  EXPIRED
  FAILED
}

enum VerificationAction {
  APPROVE
  REJECT
  REQUEST_CHANGES
}

enum ReputationTier {
  NEW
  BEGINNER
  INTERMEDIATE
  ADVANCED
  TRUSTED
  ELITE
}

enum DisputeResolution {
  PENDING
  EMPLOYER_FAVOR
  WORKER_FAVOR
  SPLIT
  ESCALATED
  WITHDRAWN
}
```

---

### Types

```graphql
type Task {
  id: ID!
  title: String!
  description: String
  rewardSats: BigInt!
  currency: String!
  state: TaskState!
  
  # Parties
  employerPubkey: Hex!
  workerPubkey: Hex
  
  # Funding
  funding: Funding
  
  # Proof
  proofUrl: String
  proofHash: Hex
  proofNostrEventId: String
  proofArchiveUrl: String
  
  # Verification
  verifiedBy: Hex
  verifiedAt: DateTime
  verificationReason: String
  
  # Metadata
  deadline: DateTime
  metadata: JSON
  nostrEventId: String
  
  # Timestamps
  createdAt: DateTime!
  updatedAt: DateTime!
  claimedAt: DateTime
  completedAt: DateTime
  settledAt: DateTime
  
  # Relations
  dispute: Dispute
}

type Funding {
  id: ID!
  taskId: ID!
  
  # Payment rail
  mode: FundingMode!
  provider: String!
  
  # Lightning
  invoice: String
  invoiceHash: Hex
  preimageHash: Hex
  holdInvoiceId: String
  
  # Amount & expiry
  amountSats: BigInt!
  expiresAt: DateTime
  
  # On-chain / Swap
  onchainAddress: String
  swapId: String
  lockupScript: String
  timeoutBlock: Int
  
  # Status
  status: FundingStatus!
  paymentReceivedAt: DateTime
  settledAt: DateTime
  cancelledAt: DateTime
  
  # Metadata
  externalId: String
  externalMetadata: JSON
  
  # Timestamps
  createdAt: DateTime!
  updatedAt: DateTime!
}

type LightningFundingResponse {
  fundingId: ID!
  invoice: String!
  invoiceHash: Hex!
  amountSats: BigInt!
  expiresAt: DateTime!
  holdInvoiceId: String!
  qrCode: String
}

type SubmarineFundingResponse {
  fundingId: ID!
  provider: String!
  swapId: String!
  onchainAddress: String!
  amountSats: BigInt!
  lockupScript: String!
  timeoutBlock: Int!
  boltzPaymentRequest: String
}

type Settlement {
  taskId: ID!
  state: TaskState!
  preimage: Hex
  settledAt: DateTime!
  paymentHash: Hex!
  workerReceivedSats: BigInt!
}

type Refund {
  taskId: ID!
  state: TaskState!
  refundedAt: DateTime!
  amountSats: BigInt!
  holdInvoiceCancelled: Boolean!
}

type Reputation {
  pubkey: Hex!
  score: Int!
  tier: ReputationTier!
  
  # Employer stats
  tasksCreated: Int!
  tasksFunded: Int!
  totalSatsPaid: BigInt!
  
  # Worker stats
  tasksClaimed: Int!
  tasksCompleted: Int!
  totalSatsEarned: BigInt!
  
  # Quality
  disputesTotal: Int!
  disputesWon: Int!
  avgCompletionTimeHours: Float!
  avgRating: Float!
  
  # Badges
  badges: [String!]!
  
  # Penalties
  penaltyPoints: Int!
  suspendedUntil: DateTime
  
  # Timestamps
  firstSeenAt: DateTime!
  lastActiveAt: DateTime!
  updatedAt: DateTime!
}

type RecentTask {
  taskId: ID!
  role: String!
  state: TaskState!
  completedAt: DateTime
}

type Dispute {
  id: ID!
  taskId: ID!
  
  # Parties
  initiatedBy: Hex!
  respondent: Hex!
  
  # Reason
  reason: String!
  evidenceUrls: [String!]
  
  # Arbitration
  arbitratorPubkey: Hex
  resolution: DisputeResolution!
  resolutionReason: String
  
  # Outcome
  winner: Hex
  fundsDistribution: JSON
  
  # Timestamps
  createdAt: DateTime!
  resolvedAt: DateTime
  
  # Nostr
  nostrEventId: String
}

type Stats {
  totalTasks: Int!
  totalSettledSats: BigInt!
  activeTasks: Int!
  avgSettlementTimeHours: Float!
  nodeInfo: NodeInfo!
}

type NodeInfo {
  pubkey: Hex!
  channels: Int!
  capacitySats: BigInt!
  localBalanceSats: BigInt!
}

type Health {
  status: String!
  services: ServiceHealth!
  version: String!
}

type ServiceHealth {
  database: String!
  ldkNode: String!
  nostrRelay: String!
  boltzApi: String!
}
```

---

### Input Types

```graphql
input CreateTaskInput {
  title: String!
  description: String
  rewardSats: BigInt!
  currency: String!
  custodyMode: FundingMode!
  deadline: DateTime
  employerPubkey: Hex!
  metadata: JSON
}

input FundTaskInput {
  mode: FundingMode!
  pubkey: Hex
  nodeUri: String
}

input ClaimTaskInput {
  workerPubkey: Hex!
  invoice: String!
  message: String
}

input SubmitProofInput {
  workerPubkey: Hex!
  proofUrl: String!
  proofHash: Hex!
  nostrEventId: String!
  nostrSig: String!
  metadata: JSON
}

input VerifyProofInput {
  verifierPubkey: Hex!
  action: VerificationAction!
  reason: String!
  signature: String!
}

input SettleTaskInput {
  action: String!
  preimageRelease: Boolean!
}

input CancelTaskInput {
  reason: String!
  signature: String!
}

input ResolveDisputeInput {
  disputeId: ID!
  resolution: DisputeResolution!
  resolutionReason: String!
  fundsDistribution: JSON
}
```

---

### Union Types

```graphql
union FundingResponse = LightningFundingResponse | SubmarineFundingResponse
```

---

### Queries

```graphql
type Query {
  # Tasks
  task(id: ID!): Task
  tasks(
    state: TaskState
    employerPubkey: Hex
    workerPubkey: Hex
    limit: Int = 50
    offset: Int = 0
  ): [Task!]!
  
  # Reputation
  reputation(pubkey: Hex!): Reputation
  reputationLeaderboard(
    tier: ReputationTier
    limit: Int = 100
  ): [Reputation!]!
  
  # Disputes
  dispute(id: ID!): Dispute
  disputes(
    taskId: ID
    resolution: DisputeResolution
    limit: Int = 50
  ): [Dispute!]!
  
  # Statistics
  stats: Stats!
  
  # Health
  health: Health!
}
```

---

### Mutations

```graphql
type Mutation {
  # Task lifecycle
  createTask(input: CreateTaskInput!): Task!
  fundTask(taskId: ID!, input: FundTaskInput!): FundingResponse!
  claimTask(taskId: ID!, input: ClaimTaskInput!): Task!
  submitProof(taskId: ID!, input: SubmitProofInput!): Task!
  verifyProof(taskId: ID!, input: VerifyProofInput!): Task!
  settleTask(taskId: ID!, input: SettleTaskInput!): Settlement!
  cancelTask(taskId: ID!, input: CancelTaskInput!): Refund!
  
  # Disputes
  createDispute(taskId: ID!, reason: String!, evidenceUrls: [String!]): Dispute!
  resolveDispute(input: ResolveDisputeInput!): Dispute!
}
```

---

### Subscriptions

```graphql
type Subscription {
  # Task updates
  taskUpdated(taskId: ID!): Task!
  taskStateChanged(taskId: ID!): Task!
  
  # Funding events
  fundingStatusChanged(fundingId: ID!): Funding!
  
  # Proof submissions
  proofSubmitted(taskId: ID!): Task!
  
  # Settlements
  taskSettled(taskId: ID!): Settlement!
}
```

---

## Example Queries

### Create Task

```graphql
mutation CreateTask($input: CreateTaskInput!) {
  createTask(input: $input) {
    id
    title
    state
    rewardSats
    createdAt
    nostrEventId
  }
}

# Variables
{
  "input": {
    "title": "Build landing page",
    "description": "Need a responsive landing page",
    "rewardSats": "50000",
    "currency": "BTC",
    "custodyMode": "LIGHTNING_SELF",
    "employerPubkey": "abc123...",
    "deadline": "2025-10-30T00:00:00Z",
    "metadata": {
      "tags": ["web", "design"],
      "deliverables": ["URL", "source code"]
    }
  }
}
```

### Fund Task

```graphql
mutation FundTask($taskId: ID!, $input: FundTaskInput!) {
  fundTask(taskId: $taskId, input: $input) {
    ... on LightningFundingResponse {
      fundingId
      invoice
      invoiceHash
      amountSats
      expiresAt
      qrCode
    }
    ... on SubmarineFundingResponse {
      fundingId
      swapId
      onchainAddress
      amountSats
      timeoutBlock
    }
  }
}

# Variables
{
  "taskId": "task_abc123",
  "input": {
    "mode": "LIGHTNING_SELF",
    "pubkey": "abc123..."
  }
}
```

### Get Task

```graphql
query GetTask($id: ID!) {
  task(id: $id) {
    id
    title
    description
    rewardSats
    state
    employerPubkey
    workerPubkey
    funding {
      id
      mode
      status
      invoiceHash
      amountSats
      expiresAt
    }
    proofUrl
    proofHash
    verifiedAt
    createdAt
    updatedAt
  }
}

# Variables
{
  "id": "task_abc123"
}
```

### Submit Proof

```graphql
mutation SubmitProof($taskId: ID!, $input: SubmitProofInput!) {
  submitProof(taskId: $taskId, input: $input) {
    id
    state
    proofUrl
    proofHash
    completedAt
  }
}

# Variables
{
  "taskId": "task_abc123",
  "input": {
    "workerPubkey": "worker123...",
    "proofUrl": "https://example.com/deliverable",
    "proofHash": "sha256hash...",
    "nostrEventId": "event_proof123",
    "nostrSig": "signature...",
    "metadata": {
      "files": ["index.html"],
      "demo_url": "https://demo.example.com"
    }
  }
}
```

### Get Reputation

```graphql
query GetReputation($pubkey: Hex!) {
  reputation(pubkey: $pubkey) {
    pubkey
    score
    tier
    tasksCompleted
    totalSatsEarned
    avgCompletionTimeHours
    badges
  }
}

# Variables
{
  "pubkey": "abc123..."
}
```

### Subscribe to Task Updates

```graphql
subscription TaskUpdates($taskId: ID!) {
  taskUpdated(taskId: $taskId) {
    id
    state
    funding {
      status
    }
    updatedAt
  }
}

# Variables
{
  "taskId": "task_abc123"
}
```

---

## Error Handling

GraphQL errors follow standard format:

```json
{
  "errors": [
    {
      "message": "Nostr signature verification failed",
      "extensions": {
        "code": "INVALID_SIGNATURE",
        "details": {
          "expected_pubkey": "abc123...",
          "provided_pubkey": "def456..."
        }
      },
      "path": ["createTask"]
    }
  ],
  "data": null
}
```

**Error Codes:**
- `INVALID_SIGNATURE`: Nostr authentication failed
- `TASK_NOT_FOUND`: Task ID doesn't exist
- `INVALID_STATE`: Task not in correct state for operation
- `FUNDING_EXPIRED`: Invoice expired
- `INSUFFICIENT_LIQUIDITY`: LDK node cannot route payment
- `SWAP_FAILED`: Boltz swap failed
- `PROOF_INVALID`: Proof signature or content invalid
- `UNAUTHORIZED`: Not authorized for operation
- `RATE_LIMIT_EXCEEDED`: Too many requests

---

## Rate Limiting

Rate limits enforced per pubkey:
- **Anonymous queries**: 10/minute
- **Authenticated queries**: 100/minute
- **Mutations (invoice creation)**: 5/hour
- **Mutations (proof submission)**: 10/hour

Rate limit info returned in extensions:

```json
{
  "extensions": {
    "rateLimit": {
      "limit": 100,
      "remaining": 95,
      "reset": 1697462400
    }
  }
}
```

---

## Nostr Integration

All state-changing mutations publish corresponding Nostr events:

**Task Created (Kind 30078):**
```json
{
  "kind": 30078,
  "pubkey": "<employer_pubkey>",
  "content": "{\"title\":\"Build landing page\",\"reward\":50000}",
  "tags": [
    ["d", "task_abc123"],
    ["t", "web"],
    ["amount", "50000"]
  ]
}
```

**Proof Submitted (Kind 30079):**
```json
{
  "kind": 30079,
  "pubkey": "<worker_pubkey>",
  "content": "{\"proof_url\":\"https://...\",\"task_id\":\"task_abc123\"}",
  "tags": [
    ["e", "<task_event_id>"],
    ["p", "<employer_pubkey>"]
  ]
}
```

**Settlement (Kind 30080):**
```json
{
  "kind": 30080,
  "pubkey": "<backend_pubkey>",
  "content": "{\"task_id\":\"task_abc123\",\"settled\":true,\"amount\":50000}",
  "tags": [
    ["e", "<task_event_id>"],
    ["p", "<worker_pubkey>"],
    ["p", "<employer_pubkey>"]
  ]
}
```
