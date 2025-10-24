# Non-Custodial Lightning Escrow Backend

Complete backend architecture documentation for a trust-minimized Bitcoin/Lightning escrow system for task marketplaces.

## 📚 Documentation Index

### Core Architecture
- **[ARCHITECTURE.md](./ARCHITECTURE.md)** - High-level system design, components, trust model, and technology stack
- **[API_SPECIFICATION.md](./API_SPECIFICATION.md)** - Complete GraphQL API schema and query/mutation examples
- **[DATA_MODELS.md](./DATA_MODELS.md)** - Database schema, state machines, and Rust type hints
- **[CHANGELOG.md](./CHANGELOG.md)** - Migration guide from REST to GraphQL, Rust quick start

### Implementation Details  
- **[PAYMENT_FLOWS.md](./PAYMENT_FLOWS.md)** - Detailed payment flows for Lightning hold invoices, submarine swaps, and direct node-to-node
- **[SECURITY_VERIFICATION.md](./SECURITY_VERIFICATION.md)** - Cryptographic verification, authentication, authorization, and security operations
- **[OPERATIONS.md](./OPERATIONS.md)** - Infrastructure requirements, deployment procedures, monitoring, and operational runbooks
- **[EDGE_CASES.md](./EDGE_CASES.md)** - Comprehensive edge case handling and failure mode analysis

## 🎯 Quick Start

### What is This System?

A **non-custodial escrow backend** that enables trust-minimized task payments using:

- **Lightning Hold Invoices** (via LDK) - Preferred payment rail
- **Submarine Swaps** (via Boltz) - On-chain to Lightning fallback
- **Nostr Events** - Public auditability and verification
- **Reputation System** - Trust scoring without central authority

### Key Principles

1. **Non-Custodial**: Backend never holds private keys long-term
2. **Verifiable**: All state transitions published to Nostr
3. **Trust-Minimized**: Cryptographic escrows (hold invoices, HTLCs)
4. **Dispute Resolution**: Multi-party arbitration with reputation at stake

## 🔧 System Components

```
┌─────────────────────────────────────────────────────────────┐
│                     User Interfaces                          │
│  (Web App, Mobile App, CLI - not covered in this doc)       │
└─────────────────┬───────────────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────────────┐
│                  GraphQL API Layer                           │
│  • Task Management  • Funding  • Verification  • Disputes    │
└─────────────┬───────────────────────────────────────────────┘
              │
    ┌─────────┼─────────┬─────────────┬──────────────┐
    │         │         │             │              │
┌───▼───┐ ┌──▼──┐ ┌────▼─────┐ ┌─────▼──────┐ ┌────▼─────┐
│ Task  │ │Escrow│ │ Payment  │ │Reputation │ │  Nostr   │
│Manager│ │Engine│ │Coordinator│ │ Indexer  │ │Publisher │
└───┬───┘ └──┬──┘ └────┬─────┘ └─────┬──────┘ └────┬─────┘
    │        │         │              │             │
    │     ┌──▼─────────▼──┐       ┌───▼─────┐  ┌───▼─────┐
    │     │  LDK Node     │       │PostgreSQL│  │  Nostr  │
    │     │(Hold Invoices)│       │   + Redis│  │ Relays  │
    │     └───────┬───────┘       └──────────┘  └─────────┘
    │             │
    │     ┌───────▼────────┐
    │     │  Boltz API     │
    │     │(Submarine Swaps)│
    │     └────────────────┘
    │
┌───▼────────────────────────┐
│  Bitcoin Core (Full Node)  │
└────────────────────────────┘
```

## 📖 Reading Guide

### For Product Managers / Business Stakeholders
1. Start with [ARCHITECTURE.md](./ARCHITECTURE.md) - Understand the system philosophy
2. Review [PAYMENT_FLOWS.md](./PAYMENT_FLOWS.md) Flow A - See how typical payment works
3. Skim [API_SPECIFICATION.md](./API_SPECIFICATION.md) - Understand capabilities

### For Backend Engineers
1. Read [ARCHITECTURE.md](./ARCHITECTURE.md) - System components and design
2. Study [DATA_MODELS.md](./DATA_MODELS.md) - Database schema and state machines
3. Review [PAYMENT_FLOWS.md](./PAYMENT_FLOWS.md) - All payment flows in detail
4. Implement following [SECURITY_VERIFICATION.md](./SECURITY_VERIFICATION.md) - Critical security requirements

### For DevOps / Infrastructure
1. Read [OPERATIONS.md](./OPERATIONS.md) - Infrastructure and deployment
2. Study [EDGE_CASES.md](./EDGE_CASES.md) - Failure modes and recovery procedures
3. Set up monitoring per [OPERATIONS.md](./OPERATIONS.md) Monitoring section

### For Security Auditors
1. Read [SECURITY_VERIFICATION.md](./SECURITY_VERIFICATION.md) - Cryptographic verification
2. Review [EDGE_CASES.md](./EDGE_CASES.md) - Attack vectors and mitigations
3. Check [PAYMENT_FLOWS.md](./PAYMENT_FLOWS.md) - Payment security flows

## 🚀 MVP Implementation Checklist

### Phase 1: Core Escrow (4-6 weeks)

- [ ] **LDK Integration**
  - [ ] Deploy LDK node with hold invoice support
  - [ ] Implement hold invoice creation API
  - [ ] Implement settlement/cancellation API
  - [ ] Set up invoice monitoring webhooks

- [ ] **Task Management**
  - [ ] Database schema (tasks, funding, escrow_events)
  - [ ] GraphQL schema and resolvers (mutations and queries)
  - [ ] State machine implementation
  - [ ] Authorization middleware with Nostr signatures

- [ ] **Payment Flow (Lightning Only)**
  - [ ] Fund task via hold invoice
  - [ ] Monitor invoice acceptance
  - [ ] Proof submission and verification
  - [ ] Settlement to worker

- [ ] **Security Basics**
  - [ ] Nostr signature verification
  - [ ] Invoice hash verification before settlement
  - [ ] Idempotency checks
  - [ ] Rate limiting

### Phase 2: Reliability & Monitoring (2-3 weeks)

- [ ] **Monitoring**
  - [ ] Prometheus metrics
  - [ ] Grafana dashboards
  - [ ] Critical alerts (stuck holds, LDK down, low liquidity)

- [ ] **Operational Runbooks**
  - [ ] Stuck hold recovery procedure
  - [ ] Liquidity management
  - [ ] Database backup/restore

- [ ] **Edge Case Handling**
  - [ ] Hold timeout automation
  - [ ] Task deadline expiry
  - [ ] Proof archival

### Phase 3: On-Chain Fallback (2-3 weeks)

- [ ] **Boltz Integration**
  - [ ] Submarine swap creation
  - [ ] Swap monitoring webhooks
  - [ ] Timeout/refund handling

- [ ] **Enhanced UX**
  - [ ] Payment method selection
  - [ ] Estimated confirmation times
  - [ ] Fallback suggestions

### Phase 4: Disputes & Reputation (3-4 weeks)

- [ ] **Dispute System**
  - [ ] Dispute creation and arbitrator assignment
  - [ ] Evidence submission
  - [ ] Resolution workflows
  - [ ] Fund distribution based on ruling

- [ ] **Reputation Engine**
  - [ ] Score calculation
  - [ ] Tier system
  - [ ] Badge awards
  - [ ] Penalty system

### Phase 5: Nostr Integration (2-3 weeks)

- [ ] **Event Publishing**
  - [ ] Task creation events (Kind 30078)
  - [ ] Proof submission events (Kind 30079)
  - [ ] Settlement events (Kind 30080)

- [ ] **State Reconciliation**
  - [ ] Nostr event reconstruction
  - [ ] Database vs Nostr diff detection
  - [ ] Auto-reconciliation cron jobs

## 🔐 Security Priorities

### Critical (Must Have for Launch)
- ✅ Invoice hash verification before settlement
- ✅ Preimage never logged or stored in database
- ✅ Nostr signature verification on all requests
- ✅ Rate limiting on invoice creation
- ✅ Database transactions with row-level locking
- ✅ Audit event logging (append-only)

### Important (Should Have Soon)
- ⚠️ HSM for LDK key management
- ⚠️ Multi-arbitrator for high-value disputes
- ⚠️ Automated anomaly detection (Sybil pairs, arbitrator bias)
- ⚠️ Regular audit chain verification
- ⚠️ Incident response playbook

### Nice to Have (Future)
- 💡 Zero-knowledge proof of completion
- 💡 Federated backend (multiple LDK nodes)
- 💡 Privacy-preserving reputation (blinded signatures)
- 💡 Automated market making for task pricing

## 📊 Success Metrics

### Technical
- **Invoice Acceptance Rate**: >95% (Lightning payments successful)
- **Settlement Time (P95)**: <5 minutes from verification to worker payment
- **Dispute Rate**: <5% of tasks
- **Uptime**: 99.9% (LDK node + API)

### Business
- **Transaction Volume**: X sats/month settled
- **User Growth**: Y new pubkeys/month
- **Reputation Distribution**: Bell curve (most users in Intermediate/Advanced tiers)
- **Cost per Transaction**: <1% of task value (infrastructure + liquidity costs)

## 🤝 Implementation Guide

This is a backend architecture specification optimized for Rust. To implement:

1. **Set up Rust project** with recommended crates (see DATA_MODELS.md)
2. **Implement GraphQL schema** using async-graphql
3. **Integrate LDK** for Lightning hold invoices
4. **Follow security requirements** from SECURITY_VERIFICATION.md
5. **Test edge cases** from EDGE_CASES.md
6. **Deploy** following OPERATIONS.md

### Recommended Rust Stack

```toml
# Core
axum = "0.7"                    # Web framework
tokio = { version = "1", features = ["full"] }
async-graphql = "7.0"           # GraphQL
async-graphql-axum = "7.0"

# Database
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-native-tls"] }
redis = { version = "0.24", features = ["tokio-comp"] }

# Lightning/Bitcoin
lightning = "0.0.118"           # LDK
bitcoin = "0.31"

# Crypto
secp256k1 = "0.28"
sha2 = "0.10"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## 📄 License

This architecture documentation is released under MIT License. Implement as you see fit.

## ⚠️ Disclaimers

- **Not Financial Advice**: This is a technical specification, not investment advice
- **No Guarantees**: Lightning Network is experimental; funds can be lost
- **Regulatory Compliance**: Consult legal counsel for your jurisdiction
- **Not for Production**: This is a design document; implementation requires extensive testing

## 🔗 Resources

### Lightning Development
- [LDK Documentation](https://lightningdevkit.org/)
- [BOLT Specifications](https://github.com/lightning/bolts)
- [Lightning Network Whitepaper](https://lightning.network/lightning-network-paper.pdf)

### Nostr Protocol
- [NIPs (Nostr Implementation Possibilities)](https://github.com/nostr-protocol/nips)
- [Nostr Developer Resources](https://nostr-resources.com/)
- [nostr-sdk (Rust)](https://github.com/rust-nostr/nostr)

### Bitcoin/Submarine Swaps
- [Boltz Exchange API](https://docs.boltz.exchange/)
- [Submarine Swaps Explained](https://blog.muun.com/submarine-swaps/)

### Security
- [Bitcoin OpTech](https://bitcoinops.org/) 
- [Lightning Network Security](https://github.com/lightning/bolts/blob/master/00-introduction.md#security)

### Rust Resources
- [async-graphql Book](https://async-graphql.github.io/async-graphql/en/index.html)
- [Axum Web Framework](https://github.com/tokio-rs/axum)
- [Rust LDK Docs](https://docs.rs/lightning/latest/lightning/)
- [SQLx Guide](https://github.com/launchbadge/sqlx)

---

**Last Updated**: October 16, 2025  
**Version**: 1.0.0  
**API Type**: GraphQL  
**Target Language**: Rust  
**Status**: Design Phase - Not Implemented
