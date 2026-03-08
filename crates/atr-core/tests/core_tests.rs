//! Tests for atr-core types

use atr_core::chain::Chain;
use atr_core::intent::{BatchMode, IntentBatch, IntentOperation, TransactionIntent};
use atr_core::transaction::{SimulationResult, TransactionRecord, TransactionStatus};
use uuid::Uuid;

#[test]
fn test_chain_properties() {
    assert_eq!(Chain::Base.chain_id(), Some(8453));
    assert_eq!(Chain::Solana.chain_id(), None);
    assert!(Chain::Base.is_evm());
    assert!(!Chain::Solana.is_evm());
    assert!(Chain::Solana.is_solana());
    assert!(!Chain::Base.is_solana());
    assert_eq!(format!("{}", Chain::Base), "base");
    assert_eq!(format!("{}", Chain::Solana), "solana");
}

#[test]
fn test_transaction_record_lifecycle() {
    let id = Uuid::new_v4();
    let mut record = TransactionRecord::new(id, Chain::Base);

    assert_eq!(record.status, TransactionStatus::Pending);
    assert_eq!(record.id, id);
    assert!(!record.is_terminal());

    record.update_status(TransactionStatus::Simulating);
    assert_eq!(record.status, TransactionStatus::Simulating);
    assert!(!record.is_terminal());

    record.update_status(TransactionStatus::Submitted);
    assert_eq!(record.status, TransactionStatus::Submitted);
    assert!(record.finalized_at.is_none());

    record.update_status(TransactionStatus::Finalized);
    assert_eq!(record.status, TransactionStatus::Finalized);
    assert!(record.is_terminal());
    assert!(record.finalized_at.is_some());
}

#[test]
fn test_transaction_record_failed_is_terminal() {
    let mut record = TransactionRecord::new(Uuid::new_v4(), Chain::Solana);
    record.update_status(TransactionStatus::Failed);
    assert!(record.is_terminal());
    assert!(record.finalized_at.is_some());
}

#[test]
fn test_transaction_record_dropped_is_terminal() {
    let mut record = TransactionRecord::new(Uuid::new_v4(), Chain::Base);
    record.update_status(TransactionStatus::Dropped);
    assert!(record.is_terminal());
}

#[test]
fn test_intent_serialization_transfer() {
    let intent = TransactionIntent {
        id: Uuid::new_v4(),
        chain: Chain::Base,
        operation: IntentOperation::Transfer {
            to: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            amount: 1_000_000_000_000_000_000, // 1 ETH in wei
        },
        idempotency_key: Some("test-key-1".to_string()),
        max_fee: Some(100_000_000_000_000), // 0.0001 ETH
        timeout_secs: Some(300),
    };

    let json = serde_json::to_string(&intent).unwrap();
    let deserialized: TransactionIntent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, intent.id);
    assert_eq!(deserialized.chain, Chain::Base);
}

#[test]
fn test_intent_serialization_swap() {
    let intent = TransactionIntent {
        id: Uuid::new_v4(),
        chain: Chain::Solana,
        operation: IntentOperation::Swap {
            from_token: "So11111111111111111111111111111111111111112".to_string(),
            to_token: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            amount_in: 1_000_000_000,
            min_amount_out: 50_000_000,
            dex: "jupiter".to_string(),
        },
        idempotency_key: None,
        max_fee: None,
        timeout_secs: None,
    };

    let json = serde_json::to_string(&intent).unwrap();
    assert!(json.contains("swap"));
    let deserialized: TransactionIntent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.chain, Chain::Solana);
}

#[test]
fn test_intent_serialization_contract_call() {
    let intent = TransactionIntent {
        id: Uuid::new_v4(),
        chain: Chain::Base,
        operation: IntentOperation::ContractCall {
            contract: "0xabcdef1234567890abcdef1234567890abcdef12".to_string(),
            method: "a9059cbb".to_string(), // transfer(address,uint256)
            args: serde_json::json!({
                "to": "0x1234",
                "amount": 100
            }),
            value: None,
        },
        idempotency_key: None,
        max_fee: None,
        timeout_secs: None,
    };

    let json = serde_json::to_string(&intent).unwrap();
    let deserialized: TransactionIntent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.chain, Chain::Base);
}

#[test]
fn test_simulation_result() {
    let success = SimulationResult {
        success: true,
        estimated_units: Some(21000),
        estimated_fee: Some(1_000_000_000),
        error: None,
        trace: None,
    };
    assert!(success.success);

    let failure = SimulationResult {
        success: false,
        estimated_units: None,
        estimated_fee: None,
        error: Some("execution reverted".to_string()),
        trace: None,
    };
    assert!(!failure.success);
    assert!(failure.error.is_some());
}

#[test]
fn test_intent_batch() {
    let batch = IntentBatch {
        id: Uuid::new_v4(),
        intents: vec![
            TransactionIntent {
                id: Uuid::new_v4(),
                chain: Chain::Solana,
                operation: IntentOperation::Transfer {
                    to: "addr1".to_string(),
                    amount: 100,
                },
                idempotency_key: None,
                max_fee: None,
                timeout_secs: None,
            },
            TransactionIntent {
                id: Uuid::new_v4(),
                chain: Chain::Base,
                operation: IntentOperation::Transfer {
                    to: "0xaddr2".to_string(),
                    amount: 200,
                },
                idempotency_key: None,
                max_fee: None,
                timeout_secs: None,
            },
        ],
        mode: BatchMode::Sequential,
    };

    assert_eq!(batch.intents.len(), 2);
    let json = serde_json::to_string(&batch).unwrap();
    assert!(json.contains("sequential"));
}

#[test]
fn test_chain_serde_roundtrip() {
    let base_json = serde_json::to_string(&Chain::Base).unwrap();
    assert_eq!(base_json, "\"base\"");
    let parsed: Chain = serde_json::from_str(&base_json).unwrap();
    assert_eq!(parsed, Chain::Base);

    let solana_json = serde_json::to_string(&Chain::Solana).unwrap();
    assert_eq!(solana_json, "\"solana\"");
}
