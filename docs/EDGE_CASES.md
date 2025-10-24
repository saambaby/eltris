# Edge Cases & Failure Modes

## Payment Edge Cases

### 1. Partial Invoice Payment

**Scenario:** Employer attempts to pay invoice with incorrect amount.

```
Invoice: 50,000 sats
Actual payment: 45,000 sats
```

**LDK Behavior:**
- Rejects HTLC with `incorrect_payment_amount` error
- Payment fails before reaching "accepted" state
- Funds automatically returned to payer via Lightning routing

**Backend Response:**
```typescript
// LDK webhook never fires (payment rejected at protocol level)
// Funding remains in "created" state
// After expiry timeout:
await db.funding.update(fundingId, { status: 'expired' });
await db.tasks.update(taskId, { state: 'PendingFunding' }); // Can retry
```

**User Experience:**
- Employer sees payment failure in their wallet
- Backend shows: "Invoice expired. Please request a new invoice."
- No funds lost, can retry with correct amount

---

### 2. Invoice Overpayment

**Scenario:** Employer pays more than invoice amount.

```
Invoice: 50,000 sats
Actual payment: 55,000 sats
```

**LDK Behavior:**
- Accepts payment if overpayment is within tolerance (typically <1%)
- If overpayment >1%: rejects with error
- Accepted overpayments: extra sats go to worker (can't partial-settle)

**Backend Handling:**
```typescript
async function handleInvoiceAccepted(event: InvoiceEvent) {
  if (event.amount_received > event.amount_expected) {
    const overpayment = event.amount_received - event.amount_expected;
    
    if (overpayment > event.amount_expected * 0.01) {
      // >1% overpayment - shouldn't happen (LDK rejects)
      await logWarning('Unexpected overpayment', { event });
    }
    
    // Accept and credit full received amount to worker
    await db.funding.update(fundingId, {
      amount_sats: event.amount_received, // Update to actual amount
      metadata: { overpayment_sats: overpayment }
    });
  }
}
```

**Mitigation:**
- LDK configuration: set strict payment amount matching
- Warn users: "Pay exact invoice amount"

---

### 3. Multiple Payment Attempts

**Scenario:** Employer pays same invoice multiple times (e.g., app crash + retry).

**LDK Behavior:**
- First payment: Accepted and held
- Subsequent payments: Rejected with `invoice_already_paid` error
- Duplicate payments fail immediately, funds returned

**Backend State:**
```typescript
// Only first payment creates webhook event
// Subsequent attempts never reach backend
// Database state unchanged (idempotent)
```

**User Experience:**
- First payment: Success, task funded
- Retry attempts: Wallet shows "Invoice already paid" error
- No double-charge possible

---

### 4. Invoice Expiry Mid-Payment

**Scenario:** User initiates payment but invoice expires during routing.

```
T+0:00 - Invoice created (expires at T+24:00)
T+23:59 - User initiates payment
T+24:01 - HTLC arrives at LDK (invoice expired)
```

**LDK Behavior:**
- Rejects HTLC with `invoice_expired` error
- Payment fails, funds returned to payer

**Backend Handling:**
```typescript
// Cron job marks invoice as expired
setInterval(async () => {
  await db.funding.update(
    { status: 'created', expires_at: { lt: new Date() } },
    { status: 'expired' }
  );
}, 60000); // Every minute

// User must request new invoice
```

**Prevention:**
- Set conservative expiry windows (24 hours)
- Show countdown timer in UI
- Send reminder notifications at T+20 hours

---

### 5. Routing Failure After Multiple Attempts

**Scenario:** Payment tries to route but all paths fail.

**Possible Causes:**
- Insufficient liquidity along routes
- Destination node offline
- All routes require higher fees than user willing to pay
- Network partition

**LDK/Lightning Network Behavior:**
- Tries multiple routes for ~60 seconds
- If all fail: Payment fails permanently
- Funds returned to payer

**Backend Response:**
```typescript
// No webhook fired (payment never accepted)
// Funding remains "created"

// Suggest fallback options:
await notifyEmployer({
  message: "Lightning payment failed to route. Options:",
  options: [
    "Try again (network conditions may improve)",
    "Use on-chain payment (slower but more reliable)",
    "Contact support if issue persists"
  ]
});
```

**Mitigation:**
- Ensure LDK node has well-connected channels
- Monitor routing success rates
- Offer submarine swap fallback automatically

---

## Hold Invoice Edge Cases

### 6. Hold Timeout Without Settlement

**Scenario:** Invoice accepted and held, but settlement signal never arrives.

```
T+0 - Invoice accepted, funds held
T+12 - Worker submits proof
T+13 - Verification service crashes
T+24 - Hold timeout reached (no settlement signal sent)
```

**LDK Behavior:**
- Auto-cancels hold at timeout
- Fails HTLCs back to payer (refund)
- Preimage never revealed

**Backend Recovery:**
```typescript
// Detect stuck holds via cron job
async function handleStuckHolds() {
  const stuck = await db.funding.find({
    status: 'accepted',
    created_at: { lt: new Date(Date.now() - 3600000) } // 1 hour old
  });
  
  for (const funding of stuck) {
    const task = await db.tasks.findById(funding.task_id);
    
    // Check if verification completed but settlement failed
    if (task.state === 'Verified') {
      // Retry settlement
      await settleEscrow(task.id, funding.id);
    } else {
      // Allow timeout to proceed (will auto-refund)
      await logWarning('Hold timing out without verification', { task, funding });
    }
  }
}
```

**User Impact:**
- Employer: Gets refund automatically
- Worker: Wasted work (if proof was valid)
- Resolution: Manual review + re-fund task

---

### 7. Preimage Revelation Before Verification

**Scenario:** Attacker discovers preimage and claims payment before proof verified.

**Attack Vector:**
- LDK node compromise
- Preimage leaked in logs
- Side-channel attack on HSM

**Prevention:**
```typescript
// 1. Never log preimages
// 2. LDK holds preimage in encrypted storage
// 3. Settlement only triggered after verification

async function settleEscrow(taskId: string, fundingId: string) {
  const task = await db.tasks.findById(taskId);
  
  // CRITICAL: Check task state
  if (task.state !== 'Verified') {
    throw new Error('Cannot settle: task not verified');
  }
  
  // Additional: Check worker identity
  if (!task.worker_pubkey) {
    throw new Error('No worker assigned');
  }
  
  // Only then reveal preimage
  await ldk.settleInvoice(fundingId);
}
```

**Mitigation if leaked:**
- Payment settles to wrong party
- Create insurance fund to cover losses
- Reputation penalty to attacker
- Law enforcement notification (theft)

---

### 8. Settlement to Wrong Lightning Invoice

**Scenario:** Worker provides invoice, but backend settles to different invoice.

**Causes:**
- Database corruption (worker_invoice field overwritten)
- Race condition (concurrent updates)
- Malicious backend operator

**Prevention:**
```typescript
async function settleToWorker(taskId: string, fundingId: string) {
  const task = await db.tasks.findById(taskId);
  
  // Verify worker invoice matches expected worker
  const workerInvoice = await parseInvoice(task.worker_invoice);
  const expectedPubkey = task.worker_pubkey;
  
  // Cross-check invoice destination
  if (workerInvoice.payee !== expectedPubkey) {
    throw new Error('Worker invoice pubkey mismatch');
  }
  
  // Verify invoice amount matches task reward
  if (workerInvoice.amount_sats !== task.reward_sats) {
    throw new Error('Invoice amount mismatch');
  }
  
  // Settle hold invoice to worker's invoice
  await ldk.settleToInvoice(funding.hold_invoice_id, task.worker_invoice);
  
  // Audit log
  await createAuditEvent({
    type: 'settlement.worker_paid',
    task_id: taskId,
    worker_pubkey: task.worker_pubkey,
    invoice: task.worker_invoice,
    amount: task.reward_sats
  });
}
```

---

## Blockchain Edge Cases

### 9. Bitcoin Network Congestion

**Scenario:** Employer uses on-chain payment, but mempool has 500K unconfirmed txs.

**Impact:**
- Transaction takes hours/days to confirm
- Submarine swap delayed
- Task funding delayed

**Submarine Swap (Boltz) Handling:**
```typescript
// Boltz requires minimum confirmations before swap
const REQUIRED_CONFIRMATIONS = 6; // Higher during congestion

// Monitor transaction
async function monitorOnchainPayment(swapId: string) {
  const swap = await boltz.getSwapStatus(swapId);
  
  if (swap.status === 'transaction.mempool') {
    // Estimate confirmation time based on fee rate
    const feeRate = swap.transaction.feeRate;
    const estimatedBlocks = await estimateConfirmationBlocks(feeRate);
    
    await notifyEmployer({
      message: `On-chain payment detected. Estimated confirmation time: ${estimatedBlocks * 10} minutes`,
      swap_id: swapId
    });
  }
  
  // If stuck in mempool for >24 hours
  if (swap.status === 'transaction.mempool' && swap.detected_at < Date.now() - 86400000) {
    await notifyEmployer({
      message: "Transaction stuck in mempool (low fee). Options:",
      options: [
        "Wait for confirmation",
        "Cancel and use Lightning instead (via RBF)",
        "Bump fee (if wallet supports)"
      ]
    });
  }
}
```

**Prevention:**
- Recommend Lightning for time-sensitive tasks
- Display estimated on-chain confirmation time upfront
- Offer "pay more for faster confirmation" option

---

### 10. Chain Reorganization

**Scenario:** Bitcoin blockchain reorg invalidates confirmed swap transaction.

```
Block 850000: Swap tx confirmed (6 confirmations)
Block 850006: Reorg happens, swap tx disappears
```

**Probability:** Very rare (6-block reorg ~1 in 10 million)

**Boltz Handling:**
- Monitors for reorgs
- If swap tx invalidated: Marks swap as "failed"
- Refund path still available (timeout script)

**Backend Response:**
```typescript
// Boltz webhook: "swap.reorged"
async function handleSwapReorg(swapId: string) {
  const funding = await db.funding.findOne({ swap_id: swapId });
  
  // Update status
  await db.funding.update(funding.id, {
    status: 'failed',
    metadata: { failure_reason: 'blockchain_reorg' }
  });
  
  // Notify employer
  await notifyEmployer({
    message: "Rare blockchain reorg detected. Your payment may reappear in next block.",
    action: "Wait 1 hour, then contact support if not resolved"
  });
  
  // Monitor for tx re-inclusion
  setTimeout(() => checkTxReincluded(swapId), 3600000);
}
```

---

### 11. Boltz Service Outage

**Scenario:** Boltz API/service goes down mid-swap.

**Detection:**
```typescript
async function healthCheckBoltz() {
  try {
    const response = await fetch('https://api.boltz.exchange/version');
    if (!response.ok) throw new Error('Boltz API down');
    return true;
  } catch (error) {
    await sendAlert({
      severity: 'high',
      message: 'Boltz API unreachable',
      action: 'Pause submarine swap creation, notify users'
    });
    return false;
  }
}

setInterval(healthCheckBoltz, 60000); // Every minute
```

**Fallback:**
```typescript
async function createSubmarineSwap(taskId: string) {
  const boltzHealthy = await healthCheckBoltz();
  
  if (!boltzHealthy) {
    // Option 1: Queue swap for later
    await redis.lpush('pending_swaps', taskId);
    return { status: 'queued', message: 'Swap service temporarily unavailable' };
    
    // Option 2: Suggest alternative
    return {
      status: 'unavailable',
      message: 'On-chain payment temporarily unavailable. Please use Lightning.',
      alternative_methods: ['lightning_self']
    };
  }
  
  // Proceed with swap
  return await boltz.createSwap({ /* ... */ });
}
```

**User Recovery:**
- If on-chain tx already sent: User can claim refund via timeout script
- Provide refund instructions: "Use this script with your wallet"
- No funds lost (trustless HTLC)

---

## Dispute Edge Cases

### 12. Employer Disputes After Settlement

**Scenario:** Funds already released, then employer claims proof was invalid.

**Timeline:**
```
T+0 - Proof submitted
T+1 - Employer verifies (approve)
T+2 - Settlement completes, worker receives sats
T+3 - Employer disputes: "I made a mistake, proof is actually invalid"
```

**Policy:**
```typescript
async function handlePostSettlementDispute(taskId: string, reason: string) {
  const task = await db.tasks.findById(taskId);
  
  if (task.state === 'Paid') {
    // Settlement is FINAL (Lightning payments irreversible)
    return {
      status: 'rejected',
      message: 'Cannot dispute after settlement. Lightning payments are final.',
      options: [
        'If proof was truly invalid, this is a reputation issue',
        'Worker reputation will be reviewed based on historical pattern',
        'For significant fraud, contact support with evidence'
      ]
    };
  }
}
```

**Reputation Impact:**
- If pattern detected (worker submits invalid proofs repeatedly): Suspend worker
- Isolated incidents: No action (employer should verify before approving)
- Obvious fraud: Reputation penalty + possible ban

**No Chargebacks:**
- Lightning payments are final (this is a feature, not a bug)
- Encourages employers to verify carefully before approving
- Arbitration only available BEFORE settlement

---

### 13. Arbitrator Collusion

**Scenario:** Arbitrator colludes with worker to steal employer's funds.

**Attack:**
```
1. Worker submits invalid proof
2. Employer rejects → Dispute
3. Colluding arbitrator approves invalid proof
4. Funds released to worker
5. Worker and arbitrator split proceeds
```

**Prevention:**
```typescript
// Multi-arbitrator system for high-value tasks
async function escalateDispute(disputeId: string) {
  const dispute = await db.disputes.findById(disputeId);
  const task = await db.tasks.findById(dispute.task_id);
  
  // High-value tasks require multiple arbitrators
  if (task.reward_sats > 1000000) { // 0.01 BTC
    const arbitrators = await assignMultipleArbitrators(dispute, 3); // 3 arbitrators
    
    // Require 2-of-3 consensus
    return {
      resolution_type: 'multi_sig',
      required_approvals: 2,
      arbitrators: arbitrators.map(a => a.pubkey)
    };
  }
}

// Track arbitrator decisions
async function recordArbitratorDecision(arbitratorPubkey: string, decision: any) {
  await db.arbitrator_decisions.create({
    arbitrator: arbitratorPubkey,
    dispute_id: decision.dispute_id,
    ruling: decision.ruling,
    timestamp: new Date()
  });
  
  // Detect suspicious patterns
  const stats = await db.arbitrator_decisions.aggregate({
    where: { arbitrator: arbitratorPubkey },
    group_by: 'ruling'
  });
  
  // If arbitrator always rules for workers (suspicious)
  if (stats.worker_favor / stats.total > 0.95 && stats.total > 20) {
    await flagArbitrator(arbitratorPubkey, 'Unusually biased rulings');
  }
}
```

**Arbitrator Selection:**
- Randomized assignment (prevent targeting)
- Reputation-weighted (high-rep arbitrators for high-value disputes)
- Blind assignments (arbitrator doesn't know parties' identities)

---

### 14. Employer and Worker Collude to Defraud Backend

**Scenario:** Both parties cooperate to exploit backend.

**Attack Vectors:**

**A) Fake Dispute to Keep Funds Locked:**
```
Goal: Tie up backend liquidity (DoS attack)

1. Employer funds task
2. Worker claims but never submits proof
3. Employer disputes before deadline
4. Both parties refuse arbitration
5. Funds stuck indefinitely
```

**Prevention:**
```typescript
// Mandatory resolution deadlines
async function enforceDisputeDeadline(disputeId: string) {
  const dispute = await db.disputes.findById(disputeId);
  
  const DEADLINE = 7 * 24 * 3600 * 1000; // 7 days
  if (Date.now() - dispute.created_at.getTime() > DEADLINE) {
    // Auto-resolve based on default policy
    if (!dispute.respondent_response) {
      // Worker didn't respond → Refund employer
      await resolveDispute(disputeId, 'employer_favor', 'Worker non-responsive');
    } else if (!dispute.evidence_provided) {
      // No evidence → Split funds
      await resolveDispute(disputeId, 'split', 'Insufficient evidence');
    }
  }
}
```

**B) Circular Task Farming:**
```
Goal: Farm reputation via fake completed tasks

1. Employer creates task
2. Worker (same person, different pubkey) claims
3. Submit trivial proof
4. Approve immediately
5. Settle, then repeat
6. Both identities gain reputation unfairly
```

**Detection:**
```typescript
async function detectSybilBehavior(employerPubkey: string, workerPubkey: string) {
  // Check if these pubkeys frequently interact
  const interactions = await db.tasks.count({
    employer_pubkey: employerPubkey,
    worker_pubkey: workerPubkey,
    state: 'Paid'
  });
  
  const employerTotal = await db.tasks.count({
    employer_pubkey: employerPubkey,
    state: 'Paid'
  });
  
  // If >50% of employer's tasks go to same worker (suspicious)
  if (interactions / employerTotal > 0.5 && employerTotal > 10) {
    await flagSybilPair(employerPubkey, workerPubkey);
    
    // Reduce reputation gains from these interactions
    await recomputeReputation(employerPubkey, { discount_sybil: true });
    await recomputeReputation(workerPubkey, { discount_sybil: true });
  }
}
```

---

## System Failures

### 15. Database Inconsistency vs Nostr Events

**Scenario:** Database shows task as "Paid" but Nostr events show "Refunded".

**Causes:**
- Database write succeeded, Nostr publish failed
- Race condition during concurrent updates
- Malicious database modification

**Detection:**
```typescript
async function reconcileState(taskId: string) {
  const dbTask = await db.tasks.findById(taskId);
  const nostrEvents = await nostr.getEvents({
    kinds: [30078, 30079, 30080], // Task, proof, settlement events
    '#d': [taskId]
  });
  
  // Construct state from Nostr events (source of truth)
  const nostrState = reconstructTaskFromEvents(nostrEvents);
  
  if (dbTask.state !== nostrState.state) {
    await logCritical('State mismatch detected', {
      task_id: taskId,
      db_state: dbTask.state,
      nostr_state: nostrState.state
    });
    
    // Trust Nostr events (immutable, cryptographically signed)
    await db.tasks.update(taskId, { state: nostrState.state });
    
    // Investigate cause
    await triggerIncidentResponse('state_mismatch');
  }
}

// Run reconciliation daily
cron.schedule('0 3 * * *', async () => {
  const tasks = await db.tasks.find({ updated_at: { gt: thirtyDaysAgo } });
  for (const task of tasks) {
    await reconcileState(task.id);
  }
});
```

**Prevention:**
- Always publish Nostr event BEFORE updating database
- Use database transactions (atomic writes)
- Immutable event sourcing pattern

---

### 16. LDK Node Data Corruption

**Scenario:** LDK's channel state database becomes corrupted.

**Symptoms:**
- LDK crashes on startup
- Channels show incorrect balances
- Cannot create/settle invoices

**Recovery:**
```bash
# 1. STOP LDK IMMEDIATELY (prevent fund loss from bad state)
systemctl stop ldk-node

# 2. Backup current state (even if corrupted)
cp -r /var/lib/ldk /var/lib/ldk.corrupted-$(date +%s)

# 3. Attempt repair
ldk-cli check-data
ldk-cli repair-database

# 4. If repair fails, restore from backup
# WARNING: Use LATEST backup only (old state = fund loss)
rm -rf /var/lib/ldk
aws s3 sync s3://backups/ldk/latest/ /var/lib/ldk/

# 5. Restart LDK
systemctl start ldk-node

# 6. Verify channels
ldk-cli listchannels

# 7. Force-close any channels with state mismatch
ldk-cli closechannel <channel_id> --force
```

**If unrecoverable:**
- Force-close all channels (funds safe but slow recovery)
- Funds return to on-chain after timeout (CSV delay)
- Start fresh LDK instance
- All pending holds auto-refund

---

### 17. Complete Loss of Backend Infrastructure

**Scenario:** AWS region fails, all infrastructure destroyed.

**Recovery Priority:**

**1. Restore LDK Node (URGENT - prevent fund loss):**
```bash
# Deploy to new region
terraform apply -var="region=us-west-2"

# Restore LDK from S3 backup
aws s3 sync s3://backups/ldk/latest/ /var/lib/ldk/

# Start LDK
docker-compose up -d ldk-node

# Verify all channels online
ldk-cli listpeers
```

**2. Restore Database:**
```bash
# Restore PostgreSQL from latest backup
aws s3 cp s3://backups/daily/latest.dump - | pg_restore -d escrow

# Verify row counts
psql -c "SELECT 
  (SELECT COUNT(*) FROM tasks) as tasks,
  (SELECT COUNT(*) FROM funding) as funding,
  (SELECT COUNT(*) FROM escrow_events) as events;"
```

**3. Reconcile from Nostr:**
```bash
# Download all events from relays
node scripts/fetch-all-nostr-events.js

# Rebuild state from events
node scripts/reconcile-from-nostr.js --verify

# Compare with restored database
node scripts/diff-db-vs-nostr.js
```

**4. Resume Operations:**
```bash
# Deploy API servers
kubectl apply -f k8s/production/

# Run health checks
curl https://api.example.com/health

# Notify users
node scripts/publish-service-restored.js
```

**Expected Downtime:** 2-4 hours (if backup automation works)

---

## Race Conditions

### 18. Concurrent Settlement Attempts

**Scenario:** Two API servers try to settle the same task simultaneously.

**Without Locking:**
```
Server A: Checks task.state → Verified
Server B: Checks task.state → Verified
Server A: Calls ldk.settleInvoice() → Success
Server B: Calls ldk.settleInvoice() → Error (already settled)
```

**With Database Locking:**
```typescript
async function settleEscrow(taskId: string) {
  return await db.transaction(async (tx) => {
    // Row-level lock (other transactions wait)
    const task = await tx.tasks.findById(taskId, { lock: true });
    
    if (task.state !== 'Verified') {
      throw new Error('Already processed');
    }
    
    // Settle
    await ldk.settleInvoice(task.funding_id);
    
    // Update state
    await tx.tasks.update(taskId, { state: 'Paid' });
    
    // Commit transaction (releases lock)
  });
}
```

**Idempotency Key (additional safety):**
```typescript
async function settleEscrowIdempotent(taskId: string, idempotencyKey: string) {
  // Check if already processed
  const existing = await redis.get(`settle:${idempotencyKey}`);
  if (existing) {
    return JSON.parse(existing); // Return cached result
  }
  
  // Process
  const result = await settleEscrow(taskId);
  
  // Cache result
  await redis.setex(`settle:${idempotencyKey}`, 3600, JSON.stringify(result));
  
  return result;
}
```

---

### 19. Worker Claims Task During Cancellation

**Scenario:** Employer cancels task while worker is clicking "Claim" button.

```
T+0.0s: Employer → POST /tasks/123/cancel
T+0.1s: Worker → POST /tasks/123/claim
T+0.2s: Backend processes cancel
T+0.3s: Backend processes claim
```

**Race Condition Handling:**
```typescript
// Use database constraints
ALTER TABLE tasks ADD CONSTRAINT check_state_transition 
  CHECK (
    (state = 'Funded' AND worker_pubkey IS NULL) OR
    (state = 'Claimed' AND worker_pubkey IS NOT NULL)
  );

async function claimTask(taskId: string, workerPubkey: string) {
  try {
    await db.tasks.update(
      { id: taskId, state: 'Funded', worker_pubkey: null }, // WHERE clause
      { state: 'Claimed', worker_pubkey: workerPubkey, claimed_at: new Date() }
    );
  } catch (error) {
    if (error.code === '23514') { // Check constraint violation
      throw new Error('Task no longer available (may have been cancelled)');
    }
    throw error;
  }
}
```

**Outcome:**
- Either cancel succeeds (worker claim fails)
- Or claim succeeds (cancel fails)
- Never both (atomic transaction)

---

## User Error Scenarios

### 20. Worker Loses Access to Proof

**Scenario:** Worker submits proof URL, but file is deleted before employer verifies.

```
T+0 - Worker uploads to personal server: https://worker.com/proof.html
T+1 - Worker submits proof to backend
T+2 - Worker's server crashes, proof.html lost
T+3 - Employer tries to verify → 404 Not Found
```

**Prevention:**
```typescript
// Backend archives proof on submission
async function submitProof(taskId: string, proofUrl: string) {
  // Download and verify proof exists
  const proof = await fetch(proofUrl);
  if (!proof.ok) {
    throw new Error('Proof URL not accessible');
  }
  
  // Archive to backend storage
  const proofData = await proof.arrayBuffer();
  const proofHash = sha256(proofData);
  const archiveUrl = await uploadToS3(proofData, `proofs/${taskId}/${proofHash}.bin`);
  
  // Store both URLs
  await db.tasks.update(taskId, {
    proof_url: proofUrl, // Original
    proof_archive_url: archiveUrl, // Backend copy
    proof_hash: proofHash.toString('hex')
  });
}
```

**Verification Flow:**
```typescript
async function verifyProof(taskId: string) {
  const task = await db.tasks.findById(taskId);
  
  // Try original URL first
  let proof = await fetch(task.proof_url);
  
  // Fallback to archive
  if (!proof.ok && task.proof_archive_url) {
    proof = await fetch(task.proof_archive_url);
  }
  
  if (!proof.ok) {
    return {
      status: 'unavailable',
      message: 'Proof no longer accessible. Worker may need to re-upload.'
    };
  }
  
  // Verify hash matches
  const actualHash = sha256(await proof.arrayBuffer()).toString('hex');
  if (actualHash !== task.proof_hash) {
    throw new Error('Proof tampered with (hash mismatch)');
  }
  
  return { status: 'ok', data: proof };
}
```

---

### 21. Employer Loses Nostr Private Key

**Scenario:** Employer forgets/loses private key, can't cancel or verify tasks.

**Impact:**
- Cannot sign requests to verify/cancel tasks
- Tasks remain in limbo

**Recovery Options:**

**Option 1: Timeout-based Resolution**
```typescript
// Auto-refund if employer non-responsive
async function handleAbandonedTask(taskId: string) {
  const task = await db.tasks.findById(taskId);
  
  // If proof submitted but no verification for 7 days
  if (task.state === 'Claimed' && 
      task.proof_url && 
      Date.now() - task.completed_at.getTime() > 7 * 86400000) {
    
    // Auto-approve and settle (benefit of doubt to worker)
    await verifyProof(taskId, {
      action: 'approve',
      reason: 'Auto-approved due to employer non-response',
      verifier: 'system'
    });
  }
}
```

**Option 2: Recovery via NIP-05**
```typescript
// If employer verified NIP-05 (email), allow recovery
async function requestKeyRecovery(nip05: string) {
  // Send recovery link to email
  // Employer proves ownership of email
  // Issue temporary delegation key
  
  const recoveryKey = generateNostrKeypair();
  await db.users.update({ nip05 }, {
    recovery_pubkey: recoveryKey.pubkey,
    recovery_expires_at: new Date(Date.now() + 86400000) // 24 hours
  });
  
  // Recovery key can only cancel/verify, not withdraw funds
}
```

**Prevention:**
- Educate users to backup keys
- Offer optional key storage (encrypted with password)
- Recommend hardware wallets for high-value users

---

### Summary Matrix

| Edge Case | Severity | Fund Risk | Recovery | Prevention |
|-----------|----------|-----------|----------|------------|
| Partial payment | Low | None | Auto-reject | Enforce exact amount |
| Routing failure | Medium | None | Retry/fallback | Good liquidity |
| Hold timeout | Medium | None | Auto-refund | Monitor stuck holds |
| Preimage leak | **Critical** | **High** | Insurance fund | HSM + audit |
| Boltz outage | Medium | Low | Timeout refund | Health checks |
| Post-settlement dispute | Low | None | Reject | User education |
| Arbitrator collusion | High | Medium | Multi-sig | Pattern detection |
| DB corruption | High | None | Restore from Nostr | Immutable events |
| LDK corruption | **Critical** | **High** | Force-close channels | Frequent backups |
| Lost employer key | Medium | None | Timeout resolution | Key backup education |
