# Operations & Deployment

## Infrastructure Requirements

### LDK Lightning Node

**Specifications:**
- **CPU**: 4 cores minimum (8 cores recommended for high volume)
- **RAM**: 8GB minimum (16GB recommended)
- **Storage**: 500GB SSD (for blockchain data + channels)
- **Network**: 1Gbps connection, static IP, ports 9735/9736 open
- **Uptime**: 99.9% required (Lightning requires always-on nodes)

**Software Stack:**
```bash
# LDK Node Setup
- ldk-node (Rust)
- Bitcoin Core (pruned mode acceptable with 50GB)
- PostgreSQL for channel state
- Redis for caching
```

**Configuration:**

```toml
# ldk-node.toml
[node]
network = "bitcoin" # or "testnet" for testing
listening_addresses = ["0.0.0.0:9735"]
announced_listen_addr = ["<your-static-ip>:9735"]

[storage]
database_path = "/var/lib/ldk/db"
network_graph_path = "/var/lib/ldk/network_graph"

[bitcoin]
rpc_host = "127.0.0.1:8332"
rpc_user = "ldk"
rpc_password = "<secure_password>"

[channels]
# Minimum channel size for hold invoices
min_channel_size_sats = 100000 # 0.001 BTC
# Target number of channels
target_channels = 50
# Max hold time for invoices
max_hold_time_seconds = 86400 # 24 hours

[fees]
# Base fee in msats
base_fee_msat = 1000
# Fee rate in parts per million
fee_rate_ppm = 100

[liquidity]
# Alert threshold
min_liquidity_sats = 10000000 # 0.1 BTC
# Auto-open channels when below threshold
auto_channel_open = true
liquidity_provider_url = "https://lsp.example.com"
```

---

### Application Backend

**Specifications:**
- **CPU**: 2-4 cores
- **RAM**: 4GB minimum
- **Storage**: 100GB (database + logs)
- **Scaling**: Horizontal (stateless API servers)

**Tech Stack:**
```yaml
# Docker Compose Example
version: '3.8'

services:
  api:
    image: escrow-api:latest
    replicas: 3
    environment:
      DATABASE_URL: postgres://user:pass@postgres:5432/escrow
      REDIS_URL: redis://redis:6379
      LDK_RPC_URL: http://ldk-node:3030
      NOSTR_RELAYS: wss://relay1.example.com,wss://relay2.example.com
      BOLTZ_API_URL: https://api.boltz.exchange
    ports:
      - "8080:8080"
    depends_on:
      - postgres
      - redis
      - ldk-node
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  postgres:
    image: postgres:15
    volumes:
      - pgdata:/var/lib/postgresql/data
    environment:
      POSTGRES_DB: escrow
      POSTGRES_USER: escrow
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    command: 
      - "postgres"
      - "-c"
      - "max_connections=200"
      - "-c"
      - "shared_buffers=2GB"

  redis:
    image: redis:7-alpine
    volumes:
      - redisdata:/data
    command: redis-server --appendonly yes

  ldk-node:
    image: ldk-node:latest
    volumes:
      - ldkdata:/var/lib/ldk
    ports:
      - "9735:9735"
      - "3030:3030" # RPC port
    environment:
      LDK_NETWORK: bitcoin
      LDK_BITCOIN_RPC: http://bitcoin-core:8332

  bitcoin-core:
    image: ruimarinho/bitcoin-core:24
    volumes:
      - bitcoindata:/home/bitcoin/.bitcoin
    ports:
      - "8333:8333"
      - "8332:8332"
    command:
      - "-server"
      - "-txindex"
      - "-prune=50000"
      - "-rpcuser=ldk"
      - "-rpcpassword=${BTC_RPC_PASSWORD}"

volumes:
  pgdata:
  redisdata:
  ldkdata:
  bitcoindata:
```

---

### Monitoring Stack

**Prometheus + Grafana:**

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'api'
    static_configs:
      - targets: ['api:8080']
  
  - job_name: 'ldk-node'
    static_configs:
      - targets: ['ldk-node:9090']
  
  - job_name: 'postgres'
    static_configs:
      - targets: ['postgres-exporter:9187']

alerting:
  alertmanagers:
    - static_configs:
        - targets: ['alertmanager:9093']

# Alerting Rules
rule_files:
  - '/etc/prometheus/alerts/*.yml'
```

**Critical Alerts:**

```yaml
# alerts/escrow.yml
groups:
  - name: escrow_critical
    interval: 30s
    rules:
      - alert: StuckHoldInvoices
        expr: escrow_stuck_holds_count > 10
        for: 5m
        labels:
          severity: high
        annotations:
          summary: "{{ $value }} hold invoices stuck for >1 hour"
          description: "Investigate LDK settlement service"
      
      - alert: LDKNodeDown
        expr: up{job="ldk-node"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "LDK node is down"
          description: "Lightning node offline - all payments blocked"
      
      - alert: LowLiquidity
        expr: ldk_local_balance_sats < 10000000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "LDK liquidity below 0.1 BTC"
          description: "Open new channels or add funds"
      
      - alert: HighInvoiceFailureRate
        expr: rate(ldk_invoice_failures_total[5m]) > 0.1
        for: 5m
        labels:
          severity: high
        annotations:
          summary: "Invoice failure rate >10%"
          description: "Check routing or liquidity issues"
      
      - alert: AuditChainBroken
        expr: escrow_audit_chain_valid == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "CRITICAL: Audit chain integrity failed"
          description: "Possible database tampering - investigate immediately"
```

**Grafana Dashboards:**

```json
// Dashboard: Escrow Overview
{
  "panels": [
    {
      "title": "Active Tasks",
      "targets": [
        {"expr": "sum(tasks_by_state{state='Funded'})"},
        {"expr": "sum(tasks_by_state{state='Claimed'})"}
      ]
    },
    {
      "title": "LDK Liquidity",
      "targets": [
        {"expr": "ldk_local_balance_sats"},
        {"expr": "ldk_remote_balance_sats"}
      ]
    },
    {
      "title": "Settlement Time (p95)",
      "targets": [
        {"expr": "histogram_quantile(0.95, rate(settlement_duration_seconds[5m]))"}
      ]
    },
    {
      "title": "Hold Invoice Status",
      "targets": [
        {"expr": "funding_by_status{status='accepted'}"},
        {"expr": "funding_by_status{status='settled'}"}
      ]
    }
  ]
}
```

---

## Deployment Procedures

### Initial Deployment

**1. Infrastructure Setup:**

```bash
# 1. Provision servers (AWS/GCP/on-prem)
terraform apply -var-file=production.tfvars

# 2. Install dependencies
ansible-playbook -i inventory/production setup.yml

# 3. Initialize Bitcoin Core
docker-compose up -d bitcoin-core
# Wait for IBD (initial block download) - can take days
# Or use snapshot for faster sync

# 4. Initialize LDK node
docker-compose up -d ldk-node
# Fund node wallet
bitcoin-cli -rpcwallet=ldk sendtoaddress <ldk_address> 0.5
# Wait for confirmation

# 5. Open initial channels
ldk-cli openchannel <peer_pubkey> 5000000 # 0.05 BTC per channel
# Repeat for 10-20 peers
```

**2. Database Setup:**

```bash
# Run migrations
npm run migrate:production

# Seed arbitrator accounts
psql $DATABASE_URL -f seeds/arbitrators.sql

# Create indexes
psql $DATABASE_URL -f migrations/indexes.sql

# Verify
npm run db:verify
```

**3. Application Deployment:**

```bash
# Build images
docker build -t escrow-api:v1.0.0 .

# Push to registry
docker push registry.example.com/escrow-api:v1.0.0

# Deploy to Kubernetes
kubectl apply -f k8s/production/

# Verify health
kubectl get pods
kubectl logs -f deployment/escrow-api

# Run smoke tests
npm run test:e2e:production
```

**4. Monitoring Setup:**

```bash
# Deploy monitoring stack
helm install prometheus prometheus-community/kube-prometheus-stack

# Import dashboards
grafana-cli dashboard import dashboards/escrow-overview.json

# Configure alerting
kubectl apply -f monitoring/alertmanager-config.yml
```

---

### Rolling Updates

**Zero-downtime deployment:**

```bash
# 1. Deploy new version to staging
kubectl apply -f k8s/staging/ --record

# 2. Run integration tests
npm run test:integration:staging

# 3. Gradual rollout to production
kubectl set image deployment/escrow-api \
  escrow-api=registry.example.com/escrow-api:v1.1.0

# Kubernetes will perform rolling update (default: 25% at a time)

# 4. Monitor rollout
kubectl rollout status deployment/escrow-api

# 5. If issues, rollback
kubectl rollout undo deployment/escrow-api
```

**Database migrations (careful!):**

```bash
# 1. Backup database
pg_dump $DATABASE_URL > backups/pre-migration-$(date +%Y%m%d).sql

# 2. Test migration on staging
npm run migrate:staging

# 3. Run migration during low-traffic window
npm run migrate:production

# 4. Verify no errors
psql $DATABASE_URL -c "SELECT * FROM schema_migrations;"

# 5. Monitor API errors
tail -f /var/log/api/error.log
```

---

## Backup & Recovery

### Backup Strategy

**1. Database (PostgreSQL):**

```bash
# Continuous WAL archiving
# postgresql.conf
wal_level = replica
archive_mode = on
archive_command = 'aws s3 cp %p s3://backups/wal/%f'

# Daily full backups
0 2 * * * pg_dump -Fc $DATABASE_URL | aws s3 cp - s3://backups/daily/$(date +\%Y\%m\%d).dump

# Keep:
# - Daily backups for 30 days
# - Weekly backups for 1 year
# - Monthly backups indefinitely
```

**2. LDK Node:**

```bash
# Backup channel state (CRITICAL)
# LDK automatically persists to disk, but also backup:
0 * * * * tar -czf /backups/ldk-$(date +\%Y\%m\%d-\%H).tar.gz /var/lib/ldk/

# Sync to S3
*/15 * * * * aws s3 sync /var/lib/ldk/ s3://backups/ldk/ --exclude "*.lock"

# WARNING: Never restore old channel state - can cause fund loss
# Always use most recent backup
```

**3. Nostr Events (Redundant):**

```bash
# Subscribe to all relays and store locally
node scripts/backup-nostr-events.js

# Events are immutable and replicated across relays
# Local backup is for disaster recovery only
```

---

### Disaster Recovery

**Scenario 1: Database Corruption**

```bash
# 1. Stop API servers to prevent writes
kubectl scale deployment/escrow-api --replicas=0

# 2. Restore from latest backup
aws s3 cp s3://backups/daily/latest.dump - | pg_restore -d escrow

# 3. Verify restoration
psql $DATABASE_URL -c "SELECT COUNT(*) FROM tasks;"
npm run db:verify

# 4. Reconcile with Nostr events
node scripts/reconcile-from-nostr.js

# 5. Resume operations
kubectl scale deployment/escrow-api --replicas=3
```

**Scenario 2: LDK Node Failure**

```bash
# 1. Provision new node
# 2. Restore latest LDK backup
tar -xzf /backups/ldk-latest.tar.gz -C /var/lib/ldk/

# 3. Start LDK node
docker-compose up -d ldk-node

# 4. Verify channels
ldk-cli listchannels

# 5. Force-close any stuck channels
ldk-cli closechannel <channel_id> --force

# 6. Resume API operations
# Hold invoices will auto-timeout and refund if settlement impossible
```

**Scenario 3: Complete Infrastructure Loss**

```bash
# Rebuild from:
# 1. S3 backups (database, LDK state)
# 2. Nostr relays (event history)
# 3. Bitcoin blockchain (settlement verification)

# Recovery priority:
# 1. Restore LDK (prevent channel fund loss)
# 2. Restore database
# 3. Reconcile state from Nostr
# 4. Verify all pending settlements
# 5. Resume operations
```

---

## Operational Runbooks

### Runbook 1: High Number of Stuck Holds

**Symptoms:**
- Alert: `StuckHoldInvoices > 10`
- User reports: "Payment sent but task not funded"

**Diagnosis:**

```bash
# Check LDK node status
ldk-cli getinfo

# List stuck invoices
psql $DATABASE_URL << EOF
SELECT f.id, f.invoice_hash, f.created_at, t.id as task_id
FROM funding f
JOIN tasks t ON f.task_id = t.id
WHERE f.status = 'accepted'
AND f.created_at < NOW() - INTERVAL '1 hour';
EOF

# Check LDK invoice status
for invoice_hash in $(psql -tA ...); do
  ldk-cli listinvoices $invoice_hash
done
```

**Resolution:**

```bash
# If LDK shows invoices as settled but DB not updated:
# Sync state from LDK
node scripts/sync-ldk-state.js

# If invoices truly stuck:
# 1. Check LDK logs for errors
tail -f /var/lib/ldk/logs/ldk.log | grep ERROR

# 2. Restart settlement service
kubectl rollout restart deployment/settlement-service

# 3. Manually trigger settlement for stuck invoices
node scripts/settle-stuck-invoices.js --dry-run
node scripts/settle-stuck-invoices.js --execute

# 4. If unsalvageable, timeout and refund
node scripts/timeout-expired-holds.js
```

---

### Runbook 2: Low Liquidity

**Symptoms:**
- Alert: `LowLiquidity`
- Invoice creation failures
- Payment routing failures

**Diagnosis:**

```bash
# Check current liquidity
ldk-cli getinfo
# local_balance_msat: should be >10M sats

# Check channel distribution
ldk-cli listchannels | jq '.channels[] | {peer, local_balance, capacity}'

# Check recent failures
grep "insufficient liquidity" /var/lib/ldk/logs/ldk.log
```

**Resolution:**

```bash
# Short-term: Open new channels
ldk-cli openchannel <well_connected_peer> 10000000 # 0.1 BTC

# Medium-term: Rebalance existing channels
# Use circular rebalancing or submarine swaps
ldk-cli sendpayment <invoice> --max-fee-msat=10000

# Long-term: Set up LSP integration
# Configure automatic channel opens when liquidity low
```

---

### Runbook 3: Nostr Relay Issues

**Symptoms:**
- Events not publishing
- State reconciliation failures
- Users can't verify settlements

**Diagnosis:**

```bash
# Check relay connectivity
for relay in $NOSTR_RELAYS; do
  wscat -c $relay
done

# Check event publishing queue
redis-cli LLEN nostr:publish:queue

# Verify recent events
node scripts/verify-nostr-events.js --since="1 hour ago"
```

**Resolution:**

```bash
# Switch to backup relays
export NOSTR_RELAYS="wss://backup1.example.com,wss://backup2.example.com"
kubectl set env deployment/escrow-api NOSTR_RELAYS=$NOSTR_RELAYS

# Republish queued events
node scripts/republish-nostr-queue.js

# Monitor publishing success
watch -n 5 'redis-cli LLEN nostr:publish:queue'
```

---

## Security Operations

### Key Rotation

**LDK Node Keys:**

```bash
# WARNING: Rotating LDK keys is DANGEROUS
# Can cause channel fund loss if done incorrectly

# Only rotate if:
# 1. Keys compromised (confirmed)
# 2. Following incident response procedure

# Process:
# 1. Close all channels cooperatively
ldk-cli listchannels | jq -r '.channels[].channel_id' | \
  xargs -I {} ldk-cli closechannel {}

# 2. Wait for all channels to close (on-chain confirmations)
# 3. Withdraw all funds from node wallet
# 4. Generate new node with new keys
# 5. Fund new node
# 6. Open new channels
```

**API Authentication Keys:**

```bash
# Rotate Nostr backend pubkey/privkey

# 1. Generate new keypair
node scripts/generate-nostr-keys.js

# 2. Update environment variables
kubectl create secret generic nostr-keys \
  --from-literal=NOSTR_PRIVKEY=<new_privkey> \
  --dry-run=client -o yaml | kubectl apply -f -

# 3. Rolling restart
kubectl rollout restart deployment/escrow-api

# 4. Publish key rotation event to Nostr
node scripts/publish-key-rotation.js --old-pubkey=<old> --new-pubkey=<new>

# 5. Update documentation and notify users
```

---

### Incident Response Checklist

**When security incident detected:**

- [ ] **Assess severity** (low/medium/high/critical)
- [ ] **Notify team** (security@, ops@, engineering@)
- [ ] **Isolate affected systems** (firewall rules, disable endpoints)
- [ ] **Preserve evidence** (logs, database dumps, network captures)
- [ ] **Determine scope** (how many tasks/users affected?)
- [ ] **Stop ongoing attack** (rate limits, IP blocks, service pause)
- [ ] **Verify fund safety** (check LDK balances, audit recent settlements)
- [ ] **Communicate to users** (Nostr announcement, status page)
- [ ] **Remediate vulnerability** (patch, config change, key rotation)
- [ ] **Restore service** (gradual rollout, monitoring)
- [ ] **Post-mortem** (timeline, root cause, preventive measures)
- [ ] **Notify authorities** (if required by law)

---

## Performance Optimization

### Database Tuning

```sql
-- Index optimization for hot queries
CREATE INDEX CONCURRENTLY idx_tasks_state_created 
  ON tasks(state, created_at DESC) 
  WHERE state IN ('Funded', 'Claimed');

CREATE INDEX CONCURRENTLY idx_funding_status_expires 
  ON funding(status, expires_at) 
  WHERE status IN ('created', 'accepted');

-- Partitioning for escrow_events (high write volume)
CREATE TABLE escrow_events_2025_10 PARTITION OF escrow_events
  FOR VALUES FROM ('2025-10-01') TO ('2025-11-01');

-- Vacuum and analyze regularly
VACUUM ANALYZE tasks;
VACUUM ANALYZE funding;
```

### Caching Strategy

```typescript
// Redis caching for reputation (read-heavy)
async function getReputation(pubkey: string): Promise<Reputation> {
  const cached = await redis.get(`rep:${pubkey}`);
  if (cached) return JSON.parse(cached);
  
  const rep = await db.reputation.findByPk(pubkey);
  await redis.setex(`rep:${pubkey}`, 300, JSON.stringify(rep)); // 5 min TTL
  return rep;
}

// Invalidate on updates
async function updateReputation(pubkey: string, updates: any) {
  await db.reputation.update(pubkey, updates);
  await redis.del(`rep:${pubkey}`);
}
```

---

## Cost Estimation

**Monthly Infrastructure Costs (1000 active tasks/month):**

```
LDK Node Server (8 cores, 16GB RAM, 1TB SSD):     $150
API Servers (3x 4-core instances):                 $300
Database (PostgreSQL managed, 100GB):              $100
Bitcoin Core (8-core, 1TB SSD):                    $120
Monitoring (Prometheus/Grafana):                   $50
S3 Backups (500GB):                                $15
Domain & SSL:                                      $10
Lightning liquidity (opportunity cost on 1 BTC):   Variable
---------------------------------------------------------
TOTAL:                                             ~$745/month

Revenue (assuming 1% fee on 1000 tasks @ avg 50K sats):
1000 * 50000 * 0.01 = 500K sats/month (~$250 @ $50K/BTC)

Break-even: ~3000 tasks/month
```
