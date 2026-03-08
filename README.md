# Agent Transaction Router (ATR)

Production-grade transaction execution layer for AI agents on Base and Solana.

## Overview

The Agent Transaction Router solves the #1 infrastructure pain point for onchain AI agents: transaction execution reliability. Instead of spending 40-60% of engineering effort on transaction plumbing (retry logic, fee estimation, nonce management, error handling), agents can focus on intelligence.

## Features

- **Unified API**: Single interface for Base and Solana transactions
- **Pre-flight Simulation**: Detect failures before submission
- **Smart Retry**: Exponential backoff with fee escalation
- **Full Observability**: Real-time status tracking, metrics, structured logging
- **Cross-chain Coordination**: Execute coordinated intents across chains
- **Production Ready**: Proper error handling, idempotency, rate limiting

## Architecture

ATR is built as a modular Rust workspace:

- `atr-core`: Shared types and interfaces
- `atr-solana`: Solana-specific executor
- `atr-base`: Base/EVM executor
- `atr-simulator`: Pre-flight transaction simulation
- `atr-retry`: Retry engine with configurable policies
- `atr-observer`: Observability layer (metrics, logging, tracking)
- `atr-crosschain`: Cross-chain transaction coordination
- `atr-server`: HTTP/REST + WebSocket API server
- `atr-sdk`: High-level Rust SDK

## Quick Start

### Installation

```bash
cargo build --release
```

### Running the Server

```bash
cargo run --bin atr-server
```

Server starts on `http://localhost:3000`

### Using the SDK

```rust
use atr_sdk::AtrClient;
use atr_core::intent::{TransactionIntent, IntentOperation};
use atr_core::chain::Chain;

#[tokio::main]
async fn main() {
    let client = AtrClient::new("http://localhost:3000".to_string());
    
    let intent = TransactionIntent {
        id: uuid::Uuid::new_v4(),
        chain: Chain::Solana,
        operation: IntentOperation::Transfer {
            to: "recipient_address".to_string(),
            amount: 1_000_000,
        },
        idempotency_key: Some("unique_key".to_string()),
        max_fee: Some(10_000),
        timeout_secs: Some(60),
    };
    
    let id = client.submit_intent(intent).await.unwrap();
    println!("Intent submitted: {}", id);
    
    let status = client.get_status(id).await.unwrap();
    println!("Status: {:?}", status.status);
}
```

## API Endpoints

### Submit Intent
```
POST /api/v1/intents
Content-Type: application/json

{
  "id": "uuid",
  "chain": "solana",
  "operation": {
    "type": "transfer",
    "to": "address",
    "amount": 1000000
  }
}
```

### Get Status
```
GET /api/v1/intents/:id
```

### Cancel Intent
```
POST /api/v1/intents/:id/cancel
```

### Health Check
```
GET /api/v1/health
```

### Metrics
```
GET /api/v1/metrics
```

## Development Status

**Phase 1 (Current)**: Core architecture and API surface
- ✅ Workspace structure
- ✅ Core types and interfaces
- ✅ Executor trait and chain-specific stubs
- ✅ HTTP API server
- ✅ SDK client
- ✅ Observability foundation
- ⏳ Transaction building (Phase 2)
- ⏳ Actual RPC integration (Phase 2)
- ⏳ Retry execution (Phase 2)

**Phase 2 (Next)**: Full implementation
- Transaction building from intents
- RPC client integration
- Priority fee estimation
- Retry engine execution
- Cross-chain coordination
- Database persistence

## Testing

```bash
# Run all tests
cargo test

# Run with Solana test validator
solana-test-validator &
cargo test --features integration

# Run with Anvil (local EVM)
anvil &
cargo test --features integration
```

## Configuration

Configuration via environment variables:

```bash
# Solana RPC
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com

# Base RPC
BASE_RPC_URL=https://mainnet.base.org

# Server
SERVER_PORT=3000

# Retry policy
RETRY_MAX_ATTEMPTS=3
RETRY_INITIAL_BACKOFF_SECS=1
RETRY_FEE_ESCALATION=0.1
```

## License

MIT OR Apache-2.0
