//! Tests for cross-chain coordination

use atr_core::chain::Chain;
use atr_core::intent::{IntentOperation, TransactionIntent};
use atr_core::transaction::{TransactionRecord, TransactionStatus};
use atr_crosschain::{CrossChainCoordinator, CrossChainPair, PairStatus};
use uuid::Uuid;

fn make_intent(chain: Chain) -> TransactionIntent {
    TransactionIntent {
        id: Uuid::new_v4(),
        chain,
        operation: IntentOperation::Transfer {
            to: "addr".to_string(),
            amount: 100,
        },
        idempotency_key: None,
        max_fee: None,
        timeout_secs: None,
    }
}

#[test]
fn test_register_and_check_pair_pending() {
    let mut coordinator = CrossChainCoordinator::new();
    let tx_a = make_intent(Chain::Solana);
    let tx_b = make_intent(Chain::Base);
    let pair_id = Uuid::new_v4();

    coordinator.register_pair(CrossChainPair {
        id: pair_id,
        tx_a,
        tx_b,
        atomic: true,
    });

    // No records yet — should be pending
    let status = coordinator.check_pair_status(pair_id, &[]).unwrap();
    assert_eq!(status, PairStatus::Pending);
}

#[test]
fn test_pair_completed() {
    let mut coordinator = CrossChainCoordinator::new();
    let tx_a = make_intent(Chain::Solana);
    let tx_b = make_intent(Chain::Base);
    let pair_id = Uuid::new_v4();
    let a_id = tx_a.id;
    let b_id = tx_b.id;

    coordinator.register_pair(CrossChainPair {
        id: pair_id,
        tx_a,
        tx_b,
        atomic: true,
    });

    let mut rec_a = TransactionRecord::new(a_id, Chain::Solana);
    rec_a.update_status(TransactionStatus::Finalized);
    let mut rec_b = TransactionRecord::new(b_id, Chain::Base);
    rec_b.update_status(TransactionStatus::Finalized);

    let status = coordinator
        .check_pair_status(pair_id, &[rec_a, rec_b])
        .unwrap();
    assert_eq!(status, PairStatus::Completed);
}

#[test]
fn test_pair_partial_failure() {
    let mut coordinator = CrossChainCoordinator::new();
    let tx_a = make_intent(Chain::Solana);
    let tx_b = make_intent(Chain::Base);
    let pair_id = Uuid::new_v4();
    let a_id = tx_a.id;
    let b_id = tx_b.id;

    coordinator.register_pair(CrossChainPair {
        id: pair_id,
        tx_a,
        tx_b,
        atomic: true,
    });

    let mut rec_a = TransactionRecord::new(a_id, Chain::Solana);
    rec_a.update_status(TransactionStatus::Finalized);
    let mut rec_b = TransactionRecord::new(b_id, Chain::Base);
    rec_b.update_status(TransactionStatus::Failed);
    rec_b.error = Some("reverted".to_string());

    let status = coordinator
        .check_pair_status(pair_id, &[rec_a, rec_b])
        .unwrap();
    assert_eq!(status, PairStatus::PartialFailure);
}

#[test]
fn test_pair_in_progress() {
    let mut coordinator = CrossChainCoordinator::new();
    let tx_a = make_intent(Chain::Solana);
    let tx_b = make_intent(Chain::Base);
    let pair_id = Uuid::new_v4();
    let a_id = tx_a.id;
    let b_id = tx_b.id;

    coordinator.register_pair(CrossChainPair {
        id: pair_id,
        tx_a,
        tx_b,
        atomic: false,
    });

    let mut rec_a = TransactionRecord::new(a_id, Chain::Solana);
    rec_a.update_status(TransactionStatus::Finalized);
    let rec_b = TransactionRecord::new(b_id, Chain::Base);
    // rec_b still pending

    let status = coordinator
        .check_pair_status(pair_id, &[rec_a, rec_b])
        .unwrap();
    assert_eq!(status, PairStatus::InProgress);
}

#[test]
fn test_pair_not_found() {
    let coordinator = CrossChainCoordinator::new();
    let result = coordinator.check_pair_status(Uuid::new_v4(), &[]);
    assert!(result.is_err());
}
