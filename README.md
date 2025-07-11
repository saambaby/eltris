# Eltris

A high-performance, Rust-based Bitcoin arbitrage engine designed for security, reliability, and efficiency. Leveraging Lightning Network and Boltz swaps, Eltris executes seamless arbitrage strategies across multiple pairs.

## Workspace Structure

This is a Rust workspace containing the following crates:

- **`eltris-core`** - Core types and utilities shared across all crates
- **`eltris-exchange`** - Exchange integration and API clients
- **`eltris-lightning`** - Lightning Network integration and Boltz swap functionality
- **`eltris-arbitrage`** - Arbitrage engine and strategy implementation
- **`eltris-api`** - REST API server for monitoring and control
- **`eltris-cli`** - Command-line interface for managing the arbitrage engine

## Getting Started

### Prerequisites

- Rust 1.70+ (2021 edition)
- Cargo

### Build

```bash
cargo build
```

### Run

```bash
cargo run --bin eltris-cli
```

## Development

Each crate can be developed independently:

```bash
# Work on a specific crate
cd eltris-core
cargo test

# Check the entire workspace
cargo check --workspace
```

## License

MIT 