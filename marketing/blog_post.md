# I Built an Open-Source Transaction Router for AI Agents on Base

## The Problem No One Talks About

Everyone's building AI agents. Crypto Twitter is full of autonomous agents trading, minting, and deploying contracts. What nobody talks about is the unglamorous reality: **40-60% of the engineering effort goes into transaction plumbing.**

Retry logic. Fee estimation. Nonce management. Confirmation tracking. Error handling. Multi-RPC failover.

Every team rebuilds this from scratch. I decided to build it once, properly, and open-source it.

## What I Built

**ATR (Agent Transaction Router)** is a production-grade transaction execution layer for AI agents on Base. It's written in Rust, deployed on Base mainnet, and processing real transactions right now.

The idea is simple: agents declare **what** they want to do (intents), and ATR handles **how** — simulation, signing, submission, fee optimization, confirmation polling, and failure recovery.

```
Agent: "Transfer 0.01 ETH to 0xABC on Base"
  |
  v
ATR: simulate -> estimate fees -> sign -> submit -> poll -> confirm
  |
  v
Agent: gets tx_hash + confirmation status
```

One API call replaces hundreds of lines of transaction management code.

## Architecture

ATR is a 9-crate Rust workspace. Each crate has a single responsibility:

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

Why Rust? Memory safety without garbage collection. When you're signing transactions worth real money, you want the compiler catching bugs, not your users.

## Base-Specific Engineering

ATR isn't a generic wrapper. It's built specifically for Base with features that matter for L2 execution:

**EIP-1559 Fee Estimation** — Base uses EIP-1559 but with an additional layer: L1 data availability costs. Every transaction on Base has to pay for the calldata posted to Ethereum L1. ATR queries the L1 gas price oracle at `0x420000000000000000000000000000000000000F` and factors the L1 data cost into every fee estimate. Most tooling ignores this.

**Multi-RPC Failover** — Pass comma-separated RPC URLs. If one fails, ATR automatically tries the next. No manual intervention.

**Nonce Management** — Thread-safe, per-address nonce tracking with on-chain sync. Handles concurrent agent transactions without collisions.

**Confirmation Tracking** — Background poller tracks pending transactions through to finalization (12+ confirmations on Base). Agents get real-time status updates.

## It's Live on Mainnet

This is not a proof of concept. ATR is deployed and processing real transactions on Base mainnet:

- **Live API**: `https://atr-production.up.railway.app`
- **Registry Contract**: `0xb74f3537bdace1458372b5d99c781acdd10d247c`
- **Health Check**: [https://atr-production.up.railway.app/api/v1/health](https://atr-production.up.railway.app/api/v1/health)

Every transaction is verifiable on Basescan. The registry contract logs all processed intents on-chain.

## How to Use It

### Submit a Transaction

```bash
curl -X POST https://atr-production.up.railway.app/api/v1/intents \
  -H "Content-Type: application/json" \
  -H "X-API-Key: YOUR_API_KEY" \
  -d '{
    "id": "unique-uuid",
    "chain": "base",
    "operation": {
      "type": "transfer",
      "to": "0xRecipientAddress",
      "amount": 1000000000000000
    },
    "max_fee": 100000000000000,
    "timeout_secs": 120
  }'
```

### Check Status

```bash
curl https://atr-production.up.railway.app/api/v1/intents/INTENT_ID \
  -H "X-API-Key: YOUR_API_KEY"
```

That's it. Two API calls replace an entire transaction management stack.

### Using the Rust SDK

```rust
use atr_sdk::AtrClient;
use atr_core::intent::{TransactionIntent, IntentOperation};
use atr_core::chain::Chain;

let client = AtrClient::new("https://atr-production.up.railway.app", "your-api-key");

let intent = TransactionIntent {
    id: uuid::Uuid::new_v4(),
    chain: Chain::Base,
    operation: IntentOperation::Transfer {
        to: "0xRecipient".to_string(),
        amount: 1_000_000_000_000_000,
    },
    idempotency_key: Some("unique-key".to_string()),
    max_fee: Some(100_000_000_000_000),
    timeout_secs: Some(120),
};

let result = client.submit_intent(intent).await.unwrap();
println!("Tx: {:?}", result.tx_hash);
```

## What's Under the Hood

Here are some of the production details that took the most engineering effort:

**Idempotency Keys** — Every intent can carry an idempotency key. Submit the same intent twice, get the same result. Critical for agents that might retry on network timeouts.

**Pre-flight Simulation** — Before submitting any transaction, ATR runs `eth_call` to simulate it. If the simulation fails (insufficient balance, contract revert), the agent gets an error immediately instead of wasting gas.

**Rate Limiting** — Sliding window rate limiting per API key. Default is 100 requests per minute. Configurable per deployment.

**Observability** — Prometheus metrics for every operation: submissions, confirmations, failures, confirmation time histogram. SQLite persistence for full transaction history.

**Contract Deployment** — ATR handles contract creation transactions. The AtrRegistry contract on Base mainnet was deployed through ATR itself.

## The Competitive Landscape

Existing solutions like Gelato Relay, Biconomy, and thirdweb Engine solve similar problems but with key differences:

| | ATR | Gelato | Biconomy | thirdweb |
|---|---|---|---|---|
| Open Source | Yes (MIT) | Partial | No | Partial |
| Language | Rust | TypeScript | TypeScript | TypeScript |
| Self-Hosted | Yes | No | No | Yes |
| Base L1 Cost | Yes | No | No | No |
| Intent-Based | Yes | No | Yes | No |
| Vendor Lock-in | None | High | High | Medium |

ATR is the only fully open-source, Rust-native, self-hostable transaction router with Base-specific fee optimization.

## What's Next

- **Shape L2 Deployment** — Shape returns 80% of sequencer fees to contract deployers via Gasback. Deploying ATR on Shape creates a self-sustaining revenue model.
- **More Chains** — Solana support is feature-flagged and ready. Arbitrum and Optimism are straightforward additions.
- **Batch Transactions** — Atomic multi-step operations (approve + swap, bridge + deploy).
- **Webhook Callbacks** — Push confirmation status to agent endpoints instead of polling.

## Try It

The code is open source: **[github.com/michaelwinczuk/atr](https://github.com/michaelwinczuk/atr)**

The API is live: **[atr-production.up.railway.app](https://atr-production.up.railway.app/api/v1/health)**

If you're building AI agents that need reliable on-chain execution, ATR saves you months of plumbing work. Star the repo, try the API, or open a PR.

---

*Michael Winczuk builds blockchain infrastructure in Rust. Currently looking for roles in protocol engineering, infrastructure, or developer relations at L2 teams. [GitHub](https://github.com/michaelwinczuk) | DMs open on X.*
