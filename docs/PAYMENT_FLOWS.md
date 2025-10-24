# Payment Flows

## Flow A: Lightning Hold Invoice (Preferred)

This is the trust-minimized path when both parties can use Lightning.

### Actors
- **Employer (E)**: Funds the task
- **Worker (W)**: Completes task and provides proof
- **Backend (B)**: Creates hold invoice, verifies settlement, coordinates state
- **LDK Node**: Holds funds cryptographically until release signal

---

### Sequence

#### 1. Task Creation & Funding Setup

```
Employer → Backend: POST /tasks
{
  title: "Build landing page",
  reward_sats: 50000,
  custody_mode: "lightning_self"
}

Backend:
  - Creates Task (state: Draft)
  - Generates task_id
  - Publishes Nostr event (Kind 30078)

Backend → Employer: 201 Created
{
  task_id: "task_abc123",
  state: "Draft"
}
```

#### 2. Invoice Generation

```
Employer → Backend: POST /tasks/task_abc123/fund
{
  mode: "lightning_self",
  pubkey: "employer_npub..."
}

Backend → LDK:
  - generate_hold_invoice(amount: 50000)

LDK → Backend:
  - invoice: "lnbc500u1p..."
  - invoice_hash: "a1b2c3d4..."
  - hold_invoice_id: "hold_xyz123"

Backend:
  - Creates Funding record (status: created)
  - Updates Task (state: PendingFunding)
  - Stores invoice_hash (critical for verification)

Backend → Employer:
{
  invoice: "lnbc500u1p...",
  invoice_hash: "a1b2c3d4...",
  expires_at: "2025-10-16T14:00:00Z"
}
```

#### 3. Payment to Hold Invoice

```
Employer (via their node):
  - Scans/pays invoice
  - Payment routes through Lightning network

Lightning Network → LDK:
  - HTLC arrives with payment
  - LDK accepts but DOES NOT settle (holds preimage)

LDK → Backend (webhook):
{
  event: "invoice.accepted",
  invoice_hash: "a1b2c3d4...",
  amount_sats: 50000
}

Backend:
  - Updates Funding (status: accepted, payment_received_at: now)
  - Updates Task (state: Funded)
  - Creates EscrowEvent (type: invoice.accepted)
  - Publishes Nostr event (funding confirmed)
```

**CRITICAL**: At this point, funds are cryptographically locked. LDK holds the preimage. Backend cannot steal funds because it doesn't have the preimage private key outside LDK's trust boundary.

#### 4. Worker Claims Task

```
Worker → Backend: POST /tasks/task_abc123/claim
{
  worker_pubkey: "worker_npub...",
  invoice: "lnbc500u1p..." // Worker's invoice for receiving payment
}

Backend:
  - Validates worker_pubkey
  - Updates Task (worker_pubkey, state: Claimed, claimed_at)
  - Stores worker's invoice for later settlement
  - Publishes Nostr event (task claimed)
```

#### 5. Worker Submits Proof

```
Worker:
  - Completes work
  - Uploads deliverable → IPFS/CDN
  - Creates Nostr event (Kind 30079) with proof
  - Signs event with worker's key

Worker → Backend: POST /tasks/task_abc123/submit-proof
{
  worker_pubkey: "worker_npub...",
  proof_url: "https://deliverable.com/...",
  proof_hash: "sha256...",
  nostr_event_id: "event_proof123",
  nostr_sig: "schnorr_sig..."
}

Backend:
  - Verifies Nostr signature
  - Verifies proof_hash matches deliverable
  - Updates Task (proof_url, proof_hash, state: still Claimed)
  - Creates EscrowEvent (type: proof.submitted)
  - Publishes Nostr event (proof submitted)
  - Notifies Employer (via webhook/nostr)
```

#### 6. Verification

**Option A: Automatic Verification (programmatic tasks)**

```
Backend (Verification Service):
  - Downloads deliverable
  - Runs automated checks (e.g., CI/CD, linting, tests)
  - Verifies against acceptance criteria

If valid:
  - Updates Task (state: Verified, verified_by: "system")
  - Proceeds to settlement
```

**Option B: Manual Verification (Employer reviews)**

```
Employer → Backend: POST /tasks/task_abc123/verify
{
  action: "approve",
  reason: "Meets requirements",
  signature: "employer_sig..."
}

Backend:
  - Verifies employer signature
  - Updates Task (state: Verified, verified_by: employer_pubkey)
  - Creates EscrowEvent (type: proof.verified)
```

#### 7. Settlement (Fund Release)

```
Backend (Task Manager) → Escrow Engine:
  - settle_hold_invoice(hold_invoice_id: "hold_xyz123")

Escrow Engine → LDK:
  - Reveal preimage for invoice_hash "a1b2c3d4..."
  - Route payment to worker's invoice

LDK:
  - Settles HTLC by revealing preimage
  - Payment completes to Worker's node

LDK → Backend (webhook):
{
  event: "invoice.settled",
  invoice_hash: "a1b2c3d4...",
  preimage: "e5f6g7h8...",
  settled_at: "2025-10-16T16:05:00Z"
}

Backend:
  - Updates Funding (status: settled, settled_at)
  - Updates Task (state: Paid, settled_at)
  - Updates Reputation (worker: +sats, +completed, employer: +funded)
  - Creates EscrowEvent (type: settlement.completed)
  - Publishes Nostr event (settlement confirmed)
  - Sends preimage to Employer (proof of payment)
```

#### 8. Final State

```
Task: Paid
Funding: settled
Worker: Received 50000 sats
Employer: Has preimage proof
Reputation: Both parties updated
```

---

### Cancellation Flow (Before Work Claimed)

```
Employer → Backend: POST /tasks/task_abc123/cancel
{
  reason: "No longer needed",
  signature: "employer_sig..."
}

Backend → LDK:
  - cancel_hold_invoice(hold_invoice_id: "hold_xyz123")

LDK:
  - Cancels hold
  - Fails HTLCs back to payer
  - Employer receives refund via Lightning routing

Backend:
  - Updates Funding (status: cancelled, cancelled_at)
  - Updates Task (state: Refunded)
  - Creates EscrowEvent (type: refund.completed)
  - Publishes Nostr event (refund confirmed)
```

---

### Dispute Flow

```
Employer → Backend: POST /tasks/task_abc123/verify
{
  action: "reject",
  reason: "Deliverable doesn't meet specs"
}

Backend:
  - Updates Task (state: Disputed)
  - Creates Dispute record
  - Freezes hold invoice (no settlement, no cancel)
  - Notifies arbitrator

Arbitrator:
  - Reviews proof and requirements
  - Makes decision

Arbitrator → Backend: POST /disputes/dispute_xyz/resolve
{
  resolution: "worker_favor",
  reason: "Deliverable meets stated requirements"
}

Backend:
  - If worker_favor: Proceeds to settlement (flow 7)
  - If employer_favor: Proceeds to cancellation
  - If split: Settles partial amount, refunds remainder
  - Updates Reputation (penalties for bad-faith disputes)
```

---

## Flow B: On-Chain via Submarine Swap (Boltz)

For users without Lightning channels.

### Sequence

#### 1-2. Task Creation (Same as Flow A)

#### 3. Submarine Swap Setup

```
Employer → Backend: POST /tasks/task_abc123/fund
{
  mode: "onchain_submarine"
}

Backend → Boltz API: POST /createswap
{
  type: "submarine",
  pairId: "BTC/BTC",
  orderSide: "sell",
  invoice: "<backend_ldk_hold_invoice>", // Backend creates hold invoice internally
  refundPublicKey: "<employer_pubkey>"
}

Boltz → Backend:
{
  id: "swap_abc789",
  address: "bc1q...", // P2WSH address
  redeemScript: "<script_hex>",
  timeoutBlockHeight: 850000,
  expectedAmount: 50000
}

Backend:
  - Creates Funding (mode: onchain_submarine, swap_id, onchain_address)
  - Updates Task (state: PendingFunding)

Backend → Employer:
{
  onchain_address: "bc1q...",
  amount_sats: 50000,
  timeout_block: 850000,
  swap_id: "swap_abc789"
}
```

#### 4. On-Chain Payment

```
Employer (via wallet):
  - Sends 50000 sats to bc1q...

Bitcoin Network:
  - Transaction broadcast
  - Mempool detection

Boltz (monitoring address):
  - Detects mempool transaction

Boltz → Backend (webhook):
{
  event: "transaction.mempool",
  swap_id: "swap_abc789"
}

Backend:
  - Updates Funding (status: pending)

Bitcoin Network:
  - Transaction confirms (1-6 blocks)

Boltz:
  - Claims on-chain UTXO
  - Pays Lightning hold invoice to Backend's LDK

LDK → Backend (webhook):
{
  event: "invoice.accepted",
  invoice_hash: "...",
  amount_sats: 50000
}

Backend:
  - Updates Funding (status: accepted)
  - Updates Task (state: Funded)
  - Publishes Nostr event
```

#### 5-8. Same as Flow A (Claim, Proof, Verify, Settle)

**Settlement Difference:**
- Backend's LDK settles hold invoice to Worker's Lightning invoice
- Worker receives via Lightning (instant)
- Employer paid on-chain, Worker received Lightning (swap complete)

---

### Reverse Swap (Worker Wants On-Chain)

If Worker cannot receive Lightning:

```
Backend → Boltz API: POST /createswap
{
  type: "reverse",
  pairId: "BTC/BTC",
  orderSide: "buy",
  invoiceAmount: 50000,
  claimPublicKey: "<worker_pubkey>",
  preimageHash: "<hash_from_hold_invoice>"
}

Boltz → Backend:
{
  id: "reverse_swap_xyz",
  invoice: "lnbc500u1p...",
  lockupAddress: "bc1q...",
  redeemScript: "<script>",
  onchainAmount: 49500 // minus fees
}

On settlement:
  - Backend's LDK pays Boltz's invoice (using held funds)
  - Boltz sends on-chain to Worker's address
  - Worker receives on-chain (6 confirmations)
```

---

## Flow C: Direct Node-to-Node (Advanced)

For users who want maximum decentralization.

### Sequence

Employer and Worker negotiate directly:

```
1. Employer creates keysend payment or hold invoice directly to Worker
2. Backend only tracks settlement proof (preimage) via Nostr
3. Reputation updated when both parties publish settlement proof
4. No backend custody at all (backend is pure reputation/coordination layer)
```

**Trade-offs:**
- ✅ Zero trust in backend
- ✅ Direct payment (no routing through backend's LDK)
- ❌ No automated escrow (parties must trust each other or use on-chain HTLC)
- ❌ Disputes harder to resolve (no backend-held funds to arbitrate)

**Use case**: High-reputation parties who want minimal intermediation.

---

## Timeout Handling

### Hold Invoice Timeout

**Default timeout: 24 hours**

```
If invoice accepted but not settled within timeout:

LDK:
  - Auto-cancels hold
  - Fails HTLCs back to payer

Backend (cron job every 5 minutes):
  - SELECT * FROM funding WHERE status = 'accepted' AND expires_at < NOW()
  - For each: cancel_hold_invoice()
  - Update Task (state: Expired)
  - Notify parties
```

### Task Deadline Timeout

```
Backend (cron job every hour):
  - SELECT * FROM tasks WHERE deadline < NOW() AND state IN ('Claimed', 'Funded')
  - For each:
    - If Funded (not claimed): Refund employer
    - If Claimed (no proof): Refund employer, penalize worker reputation
    - Update state: Expired
```

---

## Edge Cases

### 1. Partial Payment

```
LDK receives invoice payment of 45000 sats (expected 50000):

LDK:
  - Rejects payment (invoice amount mismatch)
  - Fails HTLC

Backend:
  - No state change (Funding remains: created)
  - Notifies Employer: "Payment failed, wrong amount"
```

### 2. Routing Failure

```
Employer pays invoice, but routing fails after 60 seconds:

Lightning Network:
  - Payment fails, no HTLCs accepted

Employer's Node:
  - Refunds payment locally (no backend action needed)

Backend:
  - Funding remains: created
  - After expiry timeout: Updates to expired
  - Notifies Employer: "Invoice expired, please try again or use on-chain"
```

### 3. Boltz Swap Failure

```
Employer sends on-chain to Boltz address, but Boltz service goes down:

Boltz lockup script includes timeout:
  - After timeout_block, Employer can claim refund using refund key
  - No backend action needed (trustless HTLC)

Backend:
  - Marks Funding: failed
  - Notifies Employer with refund instructions
```

### 4. Worker Unreachable After Settlement

```
Backend settles hold invoice to Worker's provided Lightning invoice,
but Worker's node is offline:

Lightning Network:
  - Payment fails to route to Worker

LDK:
  - Hold invoice still locked (preimage not revealed yet)
  - Retries payment routing for 24 hours

If Worker comes back online:
  - Payment succeeds, preimage revealed

If Worker never comes back:
  - After timeout: Cancel hold, refund Employer
  - Penalize Worker reputation heavily
  - Task state: Expired
```

### 5. Double-Spend Attempt

```
Malicious actor tries to claim settlement twice:

Backend receives settlement request #2:
  - Checks Funding.status
  - If already "settled": Reject with error
  - Immutable escrow_events log shows first settlement
  - Nostr events provide public proof

No funds lost due to idempotency checks.
```

### 6. Preimage Leak

```
If preimage leaks before verification:

Problem: Anyone could claim payment

Mitigation:
  - LDK holds preimage in encrypted HSM
  - Preimage only revealed on explicit settle_invoice() call
  - Backend never logs preimage in plaintext
  - Task state checked: only Verified tasks trigger settlement

If leaked maliciously:
  - Payment settles to wrong party
  - Dispute opened
  - Insurance fund covers (if implemented)
  - Incident logged for security review
```

---

## Performance & Scalability

### Concurrent Hold Invoices

LDK node must have sufficient liquidity:

```
Available liquidity: 1 BTC (100M sats)
Active hold invoices: 50 tasks @ 50K sats each = 2.5M sats held

Remaining for new tasks: 97.5M sats
```

**Monitoring:**
```sql
SELECT SUM(amount_sats) FROM funding 
WHERE status = 'accepted' AND provider = 'ldk';
```

If liquidity low:
- Pause new invoice creation
- Alert ops team
- Open new channels or reduce max task amount

### Settlement Batching

For high-volume periods:

```
Instead of settling invoices one-by-one:
  - Queue settlement requests
  - Batch process every 5 minutes
  - Reduces LDK load
  - Maintains security (each settlement still atomic)
```

---

## Security Checklist

- [ ] Hold invoice preimages never logged in plaintext
- [ ] Invoice hash verified before every settlement
- [ ] Proof signatures validated (Nostr schnorr)
- [ ] Replay protection on all signed requests
- [ ] Rate limiting on invoice creation
- [ ] Timeout enforcement (auto-cancel stale holds)
- [ ] Funding amount matches task reward exactly
- [ ] Idempotency checks on settlements/refunds
- [ ] Escrow events append-only (immutable audit log)
- [ ] Nostr events published for all state changes
- [ ] Multi-sig on arbitrator actions (if human arbitration)
- [ ] HSM protection for LDK keys
- [ ] Monitoring alerts on stuck holds
- [ ] Regular liquidity balance checks
