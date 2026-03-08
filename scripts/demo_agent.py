#!/usr/bin/env python3
"""
ATR Demo Agent — Generates periodic on-chain transactions via the Agent Transaction Router.

This agent demonstrates ATR's capabilities by:
1. Performing self-transfers on Base mainnet (dust amounts)
2. Logging intents to the AtrRegistry contract
3. Checking transaction status

Usage:
    python demo_agent.py --url https://your-atr-server.com --api-key atr_xxx

Environment:
    ATR_URL       - ATR server URL (default: http://localhost:3000)
    ATR_API_KEY   - API key for authentication
"""

import os
import sys
import time
import uuid
import json
import argparse
import hashlib
from datetime import datetime

try:
    import requests
except ImportError:
    print("Install requests: pip install requests")
    sys.exit(1)

# AtrRegistry contract on Base mainnet
REGISTRY_CONTRACT = "0xb74f3537bdace1458372b5d99c781acdd10d247c"
# logIntent(bytes32,address,uint256) selector
LOG_INTENT_SELECTOR = "0x07d630ef"
# ATR wallet (self-transfer target)
ATR_WALLET = "0xa9D80d27923c437dEaA1051e1f594e2DDbe4bc91"


def submit_intent(base_url, api_key, intent):
    """Submit a transaction intent to ATR."""
    resp = requests.post(
        f"{base_url}/api/v1/intents",
        headers={"Content-Type": "application/json", "X-API-Key": api_key},
        json=intent,
        timeout=30,
    )
    return resp.json()


def check_status(base_url, api_key, intent_id):
    """Check intent status."""
    resp = requests.get(
        f"{base_url}/api/v1/intents/{intent_id}",
        headers={"X-API-Key": api_key},
        timeout=10,
    )
    return resp.json()


def self_transfer(base_url, api_key, amount_wei=10000000000000):
    """Perform a small self-transfer (default: 0.00001 ETH)."""
    intent_id = str(uuid.uuid4())
    intent = {
        "id": intent_id,
        "chain": "base",
        "operation": {
            "type": "transfer",
            "to": ATR_WALLET,
            "amount": amount_wei,
        },
        "max_fee": 500000000000000,
        "timeout_secs": 120,
    }

    print(f"[{datetime.now().isoformat()}] Submitting self-transfer: {intent_id}")
    result = submit_intent(base_url, api_key, intent)
    print(f"  Status: {result.get('status')} | Tx: {result.get('tx_hash', 'N/A')}")
    return intent_id, result


def log_to_registry(base_url, api_key, intent_id_str, to_addr, amount):
    """Log an intent to the on-chain AtrRegistry contract."""
    # Convert intent UUID to bytes32
    intent_bytes = hashlib.sha256(intent_id_str.encode()).hexdigest()
    # Encode: selector + bytes32 + address (padded) + uint256
    addr_padded = to_addr.lower().replace("0x", "").zfill(64)
    amount_hex = hex(amount)[2:].zfill(64)
    calldata = f"{LOG_INTENT_SELECTOR}{intent_bytes}{addr_padded}{amount_hex}"

    log_id = str(uuid.uuid4())
    intent = {
        "id": log_id,
        "chain": "base",
        "operation": {
            "type": "contract_call",
            "contract": REGISTRY_CONTRACT,
            "method": calldata,
            "args": {},
            "value": 0,
        },
        "max_fee": 500000000000000,
        "timeout_secs": 120,
    }

    print(f"[{datetime.now().isoformat()}] Logging to registry: {log_id}")
    result = submit_intent(base_url, api_key, intent)
    print(f"  Status: {result.get('status')} | Tx: {result.get('tx_hash', 'N/A')}")
    return result


def run_loop(base_url, api_key, interval_secs=21600):
    """Run the demo agent in a loop (default: every 6 hours)."""
    print(f"ATR Demo Agent starting")
    print(f"  Server: {base_url}")
    print(f"  Interval: {interval_secs}s ({interval_secs/3600:.1f} hours)")
    print(f"  Registry: {REGISTRY_CONTRACT}")
    print()

    cycle = 0
    while True:
        cycle += 1
        print(f"=== Cycle {cycle} ===")

        try:
            # 1. Self-transfer
            intent_id, result = self_transfer(base_url, api_key)

            if result.get("status") == "submitted":
                # 2. Wait for confirmation
                time.sleep(10)
                status = check_status(base_url, api_key, intent_id)
                print(f"  Confirmation: {status.get('status')}")

                # 3. Log to registry
                log_to_registry(base_url, api_key, intent_id, ATR_WALLET, 10000000000000)
            else:
                print(f"  Error: {result.get('error', 'unknown')}")

        except Exception as e:
            print(f"  Exception: {e}")

        print(f"  Sleeping {interval_secs}s until next cycle...")
        print()
        time.sleep(interval_secs)


def run_once(base_url, api_key):
    """Run a single cycle for testing."""
    print("ATR Demo Agent — single run")
    intent_id, result = self_transfer(base_url, api_key)
    if result.get("status") == "submitted":
        time.sleep(10)
        status = check_status(base_url, api_key, intent_id)
        print(f"  Confirmation: {status.get('status')}")
        log_to_registry(base_url, api_key, intent_id, ATR_WALLET, 10000000000000)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="ATR Demo Agent")
    parser.add_argument("--url", default=os.getenv("ATR_URL", "http://localhost:3000"))
    parser.add_argument("--api-key", default=os.getenv("ATR_API_KEY", ""))
    parser.add_argument("--interval", type=int, default=21600, help="Seconds between cycles")
    parser.add_argument("--once", action="store_true", help="Run single cycle and exit")
    args = parser.parse_args()

    if not args.api_key:
        print("Error: --api-key or ATR_API_KEY required")
        sys.exit(1)

    if args.once:
        run_once(args.url, args.api_key)
    else:
        run_loop(args.url, args.api_key, args.interval)
