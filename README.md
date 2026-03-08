# Agent Transaction Router (ATR)

Production-grade transaction execution layer for AI agents on **Base** (Ethereum L2) and **Solana**.

## The Problem

AI agents that interact with blockchains spend **40-60% of engineering effort** on transaction plumbing: retry logic, fee estimation, nonce management, confirmation tracking, and error handling. Every agent team rebuilds this from scratch.

## The Solution

ATR provides a single unified API that handles the entire transaction lifecycle. Agents declare **what** they want to do (intents), and ATR handles **how** — simulation, signing, submission, fee optimization, confirmation polling, and failure recovery.

```
Agent: "Transfer 0.01 ETH to 0xABC on Base"
  |
  v
ATR: simulate -> estimate fees -> sign -> submit -> poll -> confirm
  |
  v
Agent: gets tx_hash + confirmation status via webhook/polling
```

## Architecture

```
                    +------------------+
                    |   atr-server     |   HTTP/REST API (Axum)
                    |  Auth + Rate     |   API keys, rate limiting
                    |  Limit + Routes  |   Admin endpoints
                    +--------+---------+
                             |
              +--------------+--------------+
              |              |              |
     +--------v---+  +------v------+  +----v-------+
     |  atr-base  |  | atr-solana  |  | atr-cross  |
     |  EVM/Base  |  |   Solana    |  |  chain     |
     |  Executor  |  |  Executor   |  | Coordinator|
     +-----+------+  +------+------+  +------------+
           |                 |
     +-----v------+  +------v------+
     | Gas Est.   |  | Fee Est.    |
     | Nonce Mgr  |  | Confirm.   |
     +------------+  +-------------+
              |              |
     +--------v--------------v--------+
     |        atr-observer            |
     |  Metrics | Tracking | Storage  |
     |  (Prometheus)  (SQLite)        |
     +-----------+--------------------+
                 |
     +-----------v-----------+
     |       atr-core        |
     | Types | Traits | Errs |
     +-----------------------+
```

### Crates

| Crate | Purpose |
|-------|---------|
| `atr-core` | Shared types: intents, transactions, executor trait, errors |
| `atr-base` | Base/EVM executor: EIP-1559 tx building, alloy signing, gas estimation, nonce management |
| `atr-solana` | Solana executor: instruction building, priority fees, confirmation tracking |
| `atr-server` | Axum HTTP server: REST API, auth middleware, rate limiting, background poller |
| `atr-observer` | Observability: Prometheus metrics, SQLite persistence, in-memory tracking |
| `atr-retry` | Retry engine: exponential backoff, fee escalation, configurable policies |
| `atr-crosschain` | Cross-chain coordination: paired transactions, status tracking |
| `atr-simulator` | Pre-flight simulation: delegates to chain-specific simulators |
| `atr-sdk` | Rust SDK: high-level client for programmatic access |

## Features

### Transaction Lifecycle
- **Intent-based**: Agents declare operations (transfer, contract call, deploy), ATR handles execution
- **Pre-flight simulation**: `eth_call` / `simulateTransaction` before submission to catch failures early
- **Smart fee estimation**: EIP-1559 base fee + priority fee + L1 data cost (Base), priority fee percentile (Solana)
- **Automatic confirmation**: Background poller tracks pending transactions to finalization (12+ confirmations on Base)

### Reliability
- **Idempotency keys**: Prevent duplicate submissions
- **Nonce management**: Thread-safe per-address nonce tracking with on-chain sync
- **Multi-RPC failover**: Comma-separated RPC URLs with automatic fallback
- **Retry engine**: Exponential backoff with jitter and fee escalation (configurable)

### Security
- **API key authentication**: Per-key rate limiting, admin key management
- **Key masking**: API keys are masked in list responses
- **No key storage**: Private keys loaded from environment only, never persisted
- **Rate limiting**: Sliding window per API key (default: 100 req/min)

### Observability
- **Prometheus metrics**: Submissions, confirmations, failures, confirmation time histogram
- **SQLite persistence**: Full transaction history with status, block number, fees paid
- **Structured logging**: tracing with JSON output, per-transaction context

## Quick Start

### Prerequisites
- Rust 1.75+
- An RPC endpoint for Base and/or Solana

### Setup

```bash
# Clone and build
git clone https://github.com/michaelkernaghan/agent-tx-router.git
cd agent-tx-router
cargo build --release

# Configure
cp .env.example .env
# Edit .env with your RPC URLs and private keys

# Generate keypairs (optional)
cargo run -p atr-server --bin keygen

# Show wallet addresses (for funding)
cargo run -p atr-server --bin show-address

# Run
cargo run --bin atr-server
```

Server starts on `http://localhost:3000`. On first run, an API key is auto-generated and logged.

### Submit a Transaction

```bash
# Transfer 0.001 ETH on Base
curl -X POST http://localhost:3000/api/v1/intents \
  -H "Content-Type: application/json" \
  -H "X-API-Key: YOUR_API_KEY" \
  -d '{
    "id": "'$(uuidgen)'",
    "chain": "base",
    "operation": {
      "type": "transfer",
      "to": "0xRecipientAddress",
      "amount": 1000000000000000
    },
    "max_fee": 100000000000000,
    "timeout_secs": 120
  }'

# Check status
curl http://localhost:3000/api/v1/intents/INTENT_ID \
  -H "X-API-Key: YOUR_API_KEY"
```

### Using the Rust SDK

```rust
use atr_sdk::AtrClient;
use atr_core::intent::{TransactionIntent, IntentOperation};
use atr_core::chain::Chain;

#[tokio::main]
async fn main() {
    let client = AtrClient::new("http://localhost:3000", "your-api-key");

    let intent = TransactionIntent {
        id: uuid::Uuid::new_v4(),
        chain: Chain::Base,
        operation: IntentOperation::Transfer {
            to: "0xRecipientAddress".to_string(),
            amount: 1_000_000_000_000_000, // 0.001 ETH in wei
        },
        idempotency_key: Some("unique-key-123".to_string()),
        max_fee: Some(100_000_000_000_000),
        timeout_secs: Some(120),
    };

    let result = client.submit_intent(intent).await.unwrap();
    println!("Submitted: {} (tx: {:?})", result.id, result.tx_hash);
}
```

## API Reference

| Endpoint | Method | Auth | Description |
|----------|--------|------|-------------|
| `/api/v1/health` | GET | None | Health check + version |
| `/api/v1/metrics` | GET | None | Prometheus metrics snapshot |
| `/api/v1/intents` | POST | API Key | Submit a transaction intent |
| `/api/v1/intents/:id` | GET | API Key | Get intent status |
| `/api/v1/intents/:id/cancel` | POST | API Key | Cancel a pending intent |
| `/admin/keys` | POST | Admin Key | Create a new API key |
| `/admin/keys` | GET | Admin Key | List API keys (masked) |
| `/admin/keys/:key/revoke` | POST | Admin Key | Revoke an API key |

### Intent Operations

```json
// Transfer
{"type": "transfer", "to": "0x...", "amount": 1000000}

// Contract Call
{"type": "contract_call", "contract": "0x...", "method": "0xa9059cbb...", "value": 0}

// Deploy
{"type": "deploy", "bytecode": "0x60806040...", "constructor_args": "0x..."}

// Raw
{"type": "raw", "data": [0, 1, 2, ...]}
```

### Transaction Status Flow

```
Pending -> Simulating -> Simulated -> Submitted -> Confirmed -> Finalized
                |                         |
                v                         v
         SimulationFailed              Failed / Dropped
```

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | 3000 | Server port |
| `DATABASE_URL` | `sqlite:atr.db?mode=rwc` | SQLite path |
| `ATR_ADMIN_KEY` | (required) | Admin API key for key management |
| `RATE_LIMIT_PER_MINUTE` | 100 | Per-key rate limit |
| `POLL_INTERVAL_SECS` | 10 | Confirmation polling interval |
| `BASE_RPC_URL` | (required) | Base RPC (comma-separated for failover) |
| `BASE_PRIVATE_KEY` | (required) | Hex-encoded private key for Base |
| `SOLANA_RPC_URL` | (required) | Solana RPC (comma-separated for failover) |
| `SOLANA_PRIVATE_KEY` | (required) | Base58-encoded private key for Solana |

## Deployment

### Mainnet Checklist

1. Set production RPC URLs (Alchemy, QuickNode, or Helius)
2. Generate fresh keypairs: `cargo run -p atr-server --bin keygen`
3. Fund wallets: `cargo run -p atr-server --bin show-address`
4. Set a strong `ATR_ADMIN_KEY`
5. Adjust `RATE_LIMIT_PER_MINUTE` for expected load
6. Run behind a reverse proxy (nginx/caddy) with TLS

### Docker

```bash
docker build -t atr-server .
docker run -p 3000:3000 --env-file .env atr-server
```

## Development

```bash
# Run all tests
cargo test

# Run with logging
RUST_LOG=info cargo run --bin atr-server

# Run with debug logging
RUST_LOG=debug cargo run --bin atr-server
```

## Built With

- **Rust** — Memory-safe systems language
- **Axum** — Async HTTP framework
- **alloy** — Modern Ethereum library (EIP-1559, signing)
- **solana-sdk** — Official Solana SDK
- **SQLite** — Embedded persistence via sqlx
- **tokio** — Async runtime

## License

MIT OR Apache-2.0
