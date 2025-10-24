# Documentation Changelog

## v1.1.0 - GraphQL + Rust Conversion

**Date**: October 16, 2025

### Major Changes

#### 1. API Specification (API_SPECIFICATION.md)
- **Converted from REST to GraphQL**
  - Changed from HTTP endpoints (`POST /tasks`, `GET /tasks/:id`) to GraphQL schema
  - Added GraphQL types, queries, mutations, and subscriptions
  - Replaced status codes with GraphQL error handling
  - Added real-time subscriptions for task updates and settlements
  
- **New GraphQL Features**
  - Union types for `FundingResponse` (Lightning vs Submarine)
  - Subscription support for real-time updates
  - Scalar types: `DateTime`, `JSON`, `BigInt`, `Hex`
  - Comprehensive enum definitions for all state types

- **Authentication**
  - Updated signature format: `graphql:<operation_name>:<timestamp>:<query_hash>`
  - Still uses Nostr headers (`X-Nostr-Pubkey`, `X-Nostr-Signature`, `X-Nostr-Timestamp`)

#### 2. Data Models (DATA_MODELS.md)
- **Removed TypeScript Interfaces**
  - Deleted all `interface` and `type` definitions
  - Kept only PostgreSQL schema (SQL remains unchanged)
   
- **Added Rust Type Hints**
  - Recommended Rust crates (sqlx, async-graphql, secp256k1, etc.)
  - Example Rust type mappings without full code
  - Guidance on diesel vs sqlx

#### 3. Security Verification (SECURITY_VERIFICATION.md)
- **Removed All TypeScript Code**
  - Replaced implementation code with algorithm descriptions
  - Added Rust-specific guidance (using secp256k1, sha2, etc.)
  - Kept LDK Rust configuration examples (already in Rust)
  
- **Rust-Friendly Examples**
  - Signature verification algorithm in pseudo-Rust
  - Cryptographic primitives using Rust crate names
  - Pattern matching approach for authorization

#### 4. README (README.md)
- **Updated References**
  - Changed "REST API" to "GraphQL API"
  - Changed "TypeScript interfaces" to "Rust type hints"
  - Updated MVP checklist: "REST endpoints" → "GraphQL schema and resolvers"
  
- **Added Rust Implementation Guide**
  - Recommended Cargo.toml dependencies
  - Rust-specific resources (async-graphql book, rust-nostr, LDK docs)
  - Tech stack optimized for Rust ecosystem

#### 5. Files Unchanged
- **ARCHITECTURE.md** - Already language-agnostic
- **PAYMENT_FLOWS.md** - No code, only sequence diagrams and descriptions
- **OPERATIONS.md** - Infrastructure-focused, not code-specific
- **EDGE_CASES.md** - Scenario-based, language-agnostic

---

## Migration Guide: REST → GraphQL

### Before (REST)
```http
POST /tasks/:id/fund
Content-Type: application/json

{
  "mode": "lightning_self",
  "pubkey": "abc123..."
}
```

### After (GraphQL)
```graphql
mutation FundTask($taskId: ID!, $input: FundTaskInput!) {
  fundTask(taskId: $taskId, input: $input) {
    ... on LightningFundingResponse {
      invoice
      invoiceHash
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

### Key Differences

| Aspect | REST | GraphQL |
|--------|------|---------|
| **Endpoint** | Multiple URLs (`/tasks`, `/tasks/:id/fund`) | Single endpoint (`/graphql`) |
| **Data Fetching** | Over-fetching (fixed response structure) | Precise (client specifies fields) |
| **Real-time** | Webhooks or polling | Native subscriptions |
| **Type Safety** | Via JSON Schema or OpenAPI | Built-in schema introspection |
| **Error Handling** | HTTP status codes | Structured errors in response |
| **Batch Operations** | Multiple HTTP requests | Single query with multiple operations |

---

## Rust Implementation Quick Start

### 1. Create New Project

```bash
cargo new escrow-backend --bin
cd escrow-backend
```

### 2. Add Dependencies

```toml
[dependencies]
# Web framework
axum = { version = "0.7", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }

# GraphQL
async-graphql = "7.0"
async-graphql-axum = "7.0"

# Database
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-native-tls", "chrono", "json"] }
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }

# Lightning & Bitcoin
lightning = "0.0.118"
bitcoin = "0.31"

# Cryptography
secp256k1 = { version = "0.28", features = ["rand"] }
sha2 = "0.10"
hex = "0.4"

# Nostr
nostr-sdk = "0.29"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Utilities
anyhow = "1.0"
thiserror = "1.0"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
tracing = "0.1"
tracing-subscriber = "0.3"

# Configuration
dotenv = "0.15"
config = "0.13"
```

### 3. Project Structure

```
escrow-backend/
├── Cargo.toml
├── .env
├── migrations/
│   └── 001_initial_schema.sql
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── graphql/
│   │   ├── mod.rs
│   │   ├── schema.rs
│   │   ├── types.rs
│   │   ├── mutations.rs
│   │   ├── queries.rs
│   │   └── subscriptions.rs
│   ├── models/
│   │   ├── mod.rs
│   │   ├── task.rs
│   │   ├── funding.rs
│   │   ├── reputation.rs
│   │   └── dispute.rs
│   ├── services/
│   │   ├── mod.rs
│   │   ├── task_manager.rs
│   │   ├── escrow_engine.rs
│   │   ├── payment_coordinator.rs
│   │   ├── reputation_indexer.rs
│   │   └── nostr_publisher.rs
│   ├── auth/
│   │   ├── mod.rs
│   │   └── nostr_verify.rs
│   ├── ldk/
│   │   ├── mod.rs
│   │   └── hold_invoice.rs
│   └── db/
│       ├── mod.rs
│       └── postgres.rs
```

### 4. Key Implementation Files

#### GraphQL Schema (src/graphql/schema.rs)

```rust
use async_graphql::{Schema, EmptySubscription};
use crate::graphql::{QueryRoot, MutationRoot};

pub type AppSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub fn create_schema() -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .finish()
}
```

#### Nostr Auth (src/auth/nostr_verify.rs)

```rust
use secp256k1::{Secp256k1, schnorr, XOnlyPublicKey, Message};
use sha2::{Sha256, Digest};

pub fn verify_nostr_signature(
    pubkey: &str,
    signature: &str,
    message: &str,
) -> Result<(), AuthError> {
    let secp = Secp256k1::verification_only();
    
    let pubkey_bytes = hex::decode(pubkey)?;
    let sig_bytes = hex::decode(signature)?;
    
    let xonly_pubkey = XOnlyPublicKey::from_slice(&pubkey_bytes)?;
    let schnorr_sig = schnorr::Signature::from_slice(&sig_bytes)?;
    
    let mut hasher = Sha256::new();
    hasher.update(message.as_bytes());
    let msg_hash = hasher.finalize();
    
    let msg = Message::from_digest_slice(&msg_hash)?;
    
    secp.verify_schnorr(&schnorr_sig, &msg, &xonly_pubkey)?;
    
    Ok(())
}
```

### 5. Run Migrations

```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Run migrations
sqlx migrate run
```

### 6. Start Development

```bash
# Start database
docker-compose up -d postgres redis

# Run server
cargo run

# GraphQL Playground available at:
# http://localhost:8080/graphql
```

---

## Breaking Changes from v1.0

1. **API Protocol**: All clients must migrate from REST to GraphQL
2. **Authentication Message Format**: Signature message changed to include operation name
3. **Response Format**: No HTTP status codes; errors in GraphQL `errors` array
4. **Type System**: Strongly typed via GraphQL schema instead of JSON Schema

---

## Compatibility Notes

- **Database Schema**: No changes, fully backward compatible
- **Nostr Events**: No changes, same event kinds and structure
- **LDK Integration**: No changes, same hold invoice mechanism
- **Boltz Integration**: No changes, same submarine swap API

---

## Future Considerations

### Potential GraphQL Enhancements

1. **DataLoader** for N+1 query optimization
2. **Federation** for microservices (if scaling to multiple backends)
3. **Persisted Queries** for performance and security
4. **Schema Stitching** for modular architecture
5. **Custom Directives** for field-level authorization

### Rust-Specific Optimizations

1. **Zero-copy deserialization** with `serde_json::from_slice`
2. **Async LDK** using tokio channels
3. **Connection pooling** with deadpool-postgres
4. **Compile-time SQL checks** with sqlx macros
5. **Error handling** with thiserror for structured errors

---

## Support

For questions about GraphQL migration or Rust implementation:
- Open an issue in the repository
- See [ARCHITECTURE.md](./ARCHITECTURE.md) for system design
- See [API_SPECIFICATION.md](./API_SPECIFICATION.md) for GraphQL examples
