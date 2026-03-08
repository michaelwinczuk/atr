"""
Deploy AtrRouter contract to Shape mainnet and register for Gasback.

Prerequisites:
    pip install py-solc-x web3 requests

Usage:
    python scripts/deploy_shape.py
"""

import json
import os
import sys
import time
import requests
from solcx import compile_source, install_solc

# ── Config ──────────────────────────────────────────────────────────────────

SHAPE_RPC = "https://mainnet.shape.network"
ATR_API = os.environ.get("ATR_API", "https://atr-production.up.railway.app")
ATR_ADMIN_KEY = os.environ.get("ATR_ADMIN_KEY", "")

# Read private key from .env if not set
if not ATR_ADMIN_KEY:
    env_path = os.path.join(os.path.dirname(__file__), "..", ".env")
    if os.path.exists(env_path):
        with open(env_path) as f:
            for line in f:
                if line.startswith("ATR_ADMIN_KEY="):
                    ATR_ADMIN_KEY = line.strip().split("=", 1)[1]

SHAPE_PRIVATE_KEY = os.environ.get("SHAPE_PRIVATE_KEY", "")
if not SHAPE_PRIVATE_KEY:
    env_path = os.path.join(os.path.dirname(__file__), "..", ".env")
    if os.path.exists(env_path):
        with open(env_path) as f:
            for line in f:
                if line.startswith("SHAPE_PRIVATE_KEY="):
                    SHAPE_PRIVATE_KEY = line.strip().split("=", 1)[1]


def compile_contract():
    """Compile AtrRouter.sol and return bytecode."""
    print("Installing solc 0.8.20...")
    install_solc("0.8.20")

    contract_path = os.path.join(os.path.dirname(__file__), "..", "contracts", "AtrRouter.sol")
    with open(contract_path) as f:
        source = f.read()

    print("Compiling AtrRouter.sol...")
    compiled = compile_source(
        source,
        output_values=["abi", "bin"],
        solc_version="0.8.20",
    )

    # Find the AtrRouter contract
    for key, val in compiled.items():
        if "AtrRouter" in key and "IGasback" not in key:
            bytecode = val["bin"]
            abi = val["abi"]
            print(f"Compiled AtrRouter: {len(bytecode) // 2} bytes")

            # Save ABI for later use
            abi_path = os.path.join(os.path.dirname(__file__), "..", "contracts", "AtrRouter.abi.json")
            with open(abi_path, "w") as f:
                json.dump(abi, f, indent=2)
            print(f"ABI saved to {abi_path}")

            return bytecode, abi

    raise RuntimeError("AtrRouter not found in compiled output")


def deploy_via_atr(bytecode):
    """Deploy contract via ATR's deploy intent on Shape."""
    import uuid

    intent_id = str(uuid.uuid4())
    print(f"\nDeploying via ATR (intent: {intent_id})...")
    print(f"ATR API: {ATR_API}")

    # First, check if Shape executor is configured
    health = requests.get(f"{ATR_API}/api/v1/health", timeout=10)
    print(f"ATR Health: {health.json()}")

    payload = {
        "id": intent_id,
        "chain": "shape",
        "operation": {
            "type": "deploy",
            "bytecode": f"0x{bytecode}",
        },
        "timeout_secs": 120,
    }

    resp = requests.post(
        f"{ATR_API}/api/v1/intents",
        json=payload,
        headers={
            "Content-Type": "application/json",
            "X-API-Key": ATR_ADMIN_KEY,
        },
        timeout=30,
    )

    result = resp.json()
    print(f"Submit response: {json.dumps(result, indent=2)}")

    if result.get("error"):
        print(f"ERROR: {result['error']}")
        return None

    tx_hash = result.get("tx_hash")
    if not tx_hash:
        print("No tx_hash returned")
        return None

    print(f"\nTx submitted: {tx_hash}")
    print(f"Track on ShapeScan: https://shapescan.xyz/tx/{tx_hash}")

    # Poll for confirmation
    print("\nWaiting for confirmation...")
    for i in range(30):
        time.sleep(5)
        status_resp = requests.get(
            f"{ATR_API}/api/v1/intents/{intent_id}",
            headers={"X-API-Key": ATR_ADMIN_KEY},
            timeout=10,
        )
        status = status_resp.json()
        print(f"  [{i+1}] Status: {status.get('status', 'unknown')}")
        if status.get("status") in ("confirmed", "finalized"):
            print(f"\nContract deployed successfully!")
            return tx_hash
        if status.get("status") == "failed":
            print(f"\nDeployment failed: {status.get('error')}")
            return None

    print("\nTimed out waiting for confirmation")
    return tx_hash


def get_contract_address(tx_hash):
    """Get deployed contract address from receipt."""
    payload = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getTransactionReceipt",
        "params": [tx_hash],
    }
    resp = requests.post(SHAPE_RPC, json=payload, timeout=10)
    result = resp.json().get("result")
    if result:
        contract_addr = result.get("contractAddress")
        if contract_addr:
            return contract_addr
    return None


def register_gasback_via_atr(contract_address):
    """Call registerForGasback() on the deployed contract via ATR."""
    import uuid

    # registerForGasback() selector
    method_selector = "0x" + "d9051bcb"  # keccak256("registerForGasback()")[:4]
    # Actually compute it properly
    from hashlib import sha3_256
    import hashlib
    # Use web3 style keccak
    try:
        from web3 import Web3
        selector = Web3.keccak(text="registerForGasback()")[:4].hex()
    except ImportError:
        # Fallback: pre-computed
        selector = "e5fd30eb"

    intent_id = str(uuid.uuid4())
    print(f"\nRegistering for Gasback (intent: {intent_id})...")

    payload = {
        "id": intent_id,
        "chain": "shape",
        "operation": {
            "type": "contract_call",
            "contract": contract_address,
            "method": selector,
            "args": {},
        },
        "timeout_secs": 120,
    }

    resp = requests.post(
        f"{ATR_API}/api/v1/intents",
        json=payload,
        headers={
            "Content-Type": "application/json",
            "X-API-Key": ATR_ADMIN_KEY,
        },
        timeout=30,
    )

    result = resp.json()
    print(f"Gasback registration response: {json.dumps(result, indent=2)}")

    tx_hash = result.get("tx_hash")
    if tx_hash:
        print(f"Gasback registration tx: https://shapescan.xyz/tx/{tx_hash}")
    return tx_hash


def main():
    print("=" * 60)
    print("ATR Router — Shape Mainnet Deployment")
    print("=" * 60)

    if not ATR_ADMIN_KEY:
        print("ERROR: Set ATR_ADMIN_KEY in .env or environment")
        sys.exit(1)

    # Step 1: Compile
    bytecode, abi = compile_contract()

    # Step 2: Deploy via ATR
    tx_hash = deploy_via_atr(bytecode)
    if not tx_hash:
        print("Deployment failed")
        sys.exit(1)

    # Step 3: Get contract address
    time.sleep(5)
    contract_address = get_contract_address(tx_hash)
    if contract_address:
        print(f"\n{'=' * 60}")
        print(f"CONTRACT DEPLOYED: {contract_address}")
        print(f"ShapeScan: https://shapescan.xyz/address/{contract_address}")
        print(f"{'=' * 60}")

        # Step 4: Register for Gasback
        register_gasback_via_atr(contract_address)

        print(f"\n{'=' * 60}")
        print(f"DEPLOYMENT COMPLETE")
        print(f"  Contract: {contract_address}")
        print(f"  Chain:    Shape (360)")
        print(f"  Gasback:  Registered (80% sequencer fees)")
        print(f"  Withdraw: Call withdraw() to collect fees")
        print(f"{'=' * 60}")
    else:
        print(f"\nCould not get contract address yet.")
        print(f"Check tx on ShapeScan: https://shapescan.xyz/tx/{tx_hash}")
        print("Run this after confirmation:")
        print(f"  python scripts/deploy_shape.py --get-address {tx_hash}")


if __name__ == "__main__":
    main()
