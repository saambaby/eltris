# Backend Architecture Overview

## Philosophy

The backend acts as a **facilitator + verifier only** - never custodying funds long-term. All payments use cryptographic primitives (hold invoices, HTLCs, submarine swaps) to ensure trustless escrow.

## Core Principles

1. **Non-Custodial**: Backend never holds private keys for long-term fund storage
2. **Verifiable**: All state transitions published to Nostr for public auditability
3. **Trust-Minimized**: Users control their own nodes/wallets; backend coordinates
4. **Flexible Payment Rails**: Support Lightning (preferred) and on-chain (fallback)

## System Components

### 1. Task Manager
- Creates and tracks task lifecycle
- Coordinates state transitions (Draft → PendingFunding → Funded → Claimed → Verified → Paid)
- Validates proof submissions
- Triggers escrow releases

### 2. Escrow Engine (LDK Integration)
- Creates hold invoices with bound preimages
- Monitors invoice settlement status
- Releases or cancels hold invoices based on Task Manager signals
- Never exposes preimages outside trust boundary
- Manages Lightning node liquidity

### 3. Payment Coordinator
- Routes payments through appropriate rails (Lightning/on-chain)
- Integrates with Boltz for submarine/reverse swaps
- Handles fallback scenarios (routing failures, liquidity issues)
- Validates payment confirmations before state updates

### 4. Reputation Indexer
- Tracks completion history, dispute outcomes, settlement times
- Publishes reputation events to Nostr
- Provides reputation scores for trust-based UX improvements
- Enforces reputation-based policies (faster releases, lower deposits)

### 5. Nostr Publisher
- Publishes immutable events for:
  - Task creation/updates
  - Funding confirmations
  - Proof submissions
  - Settlement outcomes
  - Dispute resolutions
- Enables third-party verification and public audits
- Provides replay-protected event signatures

### 6. Verification Service
- Validates worker proof submissions (signatures, content, timestamps)
- Enforces business rules for task completion
- Supports manual arbitration for disputes
- Cross-references Nostr events for authenticity

## Architecture Diagram (Text)

```
┌─────────────┐      ┌──────────────┐      ┌─────────────┐
│  Employer   │      │    Worker    │      │   Nostr     │
│   (Node)    │      │    (Node)    │      │  Network    │
└──────┬──────┘      └──────┬───────┘      └──────┬──────┘
       │                    │                     │
       │ Create Task        │                     │
       ├───────────────────►│                     │
       │                    │                     │
       │ Request Invoice    │                     │
       ├───────────────────►│                     │
       │                    │                     │
       │◄───────────────────┤                     │
       │   Hold Invoice     │                     │
       │                    │                     │
       │ Pay Invoice        │                     │
       ├────────┐           │                     │
       │        │           │                     │
       │        ▼           │                     │
       │  ┌──────────────────────────┐            │
       │  │   Escrow Engine (LDK)    │            │
       │  │  ┌────────────────────┐  │            │
       │  │  │ Hold Invoice       │  │            │
       │  │  │ (Funds Locked)     │  │            │
       │  │  └────────────────────┘  │            │
       │  └──────────┬───────────────┘            │
       │             │                            │
       │             │ Funding Confirmed          │
       │             ├───────────────────────────►│
       │             │                            │
       │             │                     Publish Event
       │             │                            │
       │             │                            │
       │             │  Submit Proof              │
       │             │◄───────────────────────────┤
       │             │                            │
       │             │  Verify Proof              │
       │             │                            │
       │  ┌──────────▼───────────────┐            │
       │  │  Task Manager            │            │
       │  │  ┌────────────────────┐  │            │
       │  │  │ Proof Verified ✓   │  │            │
       │  │  └────────────────────┘  │            │
       │  └──────────┬───────────────┘            │
       │             │                            │
       │             │ Release Signal             │
       │  ┌──────────▼───────────────┐            │
       │  │   Escrow Engine (LDK)    │            │
       │  │  ┌────────────────────┐  │            │
       │  │  │ Reveal Preimage    │  │            │
       │  │  │ Settle to Worker   │  │            │
       │  │  └────────────────────┘  │            │
       │  └──────────┬───────────────┘            │
       │             │                            │
       │             │ Settlement Event           │
       │             ├───────────────────────────►│
       │             │                            │
       │             │ Payment Complete           │
       │             ├───────────────────────────►│
       │             │                     Worker Node
       │             │                            │
```

## Data Flow

1. **Task Creation**: Employer creates task → Task Manager stores in DB → Nostr event published
2. **Funding Request**: Employer requests invoice → Escrow Engine (LDK) creates hold invoice → Invoice returned
3. **Payment**: Employer pays from their node → LDK accepts & holds → Funding confirmed event
4. **Work Submission**: Worker completes & submits proof + Nostr signature → Verification Service validates
5. **Release**: Task Manager signals Escrow Engine → Preimage revealed → Payment settles to Worker
6. **Reputation**: Settlement triggers Reputation Indexer → Score updates published to Nostr

## Trust Model

- **Users trust**: Their own nodes/wallets + cryptographic primitives (HTLC, hold invoices)
- **Users verify**: All settlement events via Nostr + on-chain/Lightning proofs
- **Backend cannot**: Steal funds (no long-term custody), fake settlements (public Nostr events), double-spend (cryptographic locks)
- **Risks**: LDK node compromise (mitigated by HSM), Boltz swap failure (fallback mechanisms), routing failures (on-chain fallback)

## Technology Stack

- **Lightning**: LDK (Lightning Development Kit) for hold invoices and payment routing
- **Swaps**: Boltz API for submarine/reverse swaps (on-chain ↔ Lightning)
- **Events**: Nostr protocol for public event publishing and signatures
- **Database**: PostgreSQL for task/funding state (ephemeral, reconstructable from Nostr)
- **Security**: HSM for key management, append-only audit logs
- **API**: REST for writes, Nostr subscriptions for real-time updates

## Scaling Considerations

- **Liquidity**: Hold invoices require Lightning liquidity; integrate with LSPs or liquidity providers
- **Node Management**: Single LDK node for MVP; multi-node federation for scale
- **Rate Limiting**: Prevent DoS via invoice spam; require proof-of-work or deposits for high-frequency users
- **Batch Settlements**: For high-volume scenarios, batch multiple releases (future optimization)

## Regulatory Posture

- **Non-Custodial**: Avoid money transmitter regulations by never holding funds beyond escrow period
- **No KYC Required**: Users operate their own nodes; backend doesn't touch fiat
- **Transparent**: Public Nostr events enable regulatory auditability without backend cooperation
- **Geographic Restrictions**: Block jurisdictions where non-custodial services face unclear regulation
