//! Nonce management for concurrent Base transactions

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Manages nonces for multiple addresses with concurrent transaction support
pub struct NonceManager {
    nonces: Arc<Mutex<HashMap<String, u64>>>,
}

impl NonceManager {
    /// Create a new nonce manager
    pub fn new() -> Self {
        Self {
            nonces: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get next nonce for an address
    pub fn get_next_nonce(&self, address: &str) -> u64 {
        let mut nonces = self.nonces.lock().unwrap();
        let nonce = nonces.entry(address.to_string()).or_insert(0);
        let next = *nonce;
        *nonce += 1;
        next
    }

    /// Reset nonce for an address (after transaction failure)
    pub fn reset_nonce(&self, address: &str, nonce: u64) {
        let mut nonces = self.nonces.lock().unwrap();
        nonces.insert(address.to_string(), nonce);
    }

    /// Sync nonce with on-chain state
    pub async fn sync_nonce(&self, address: &str, on_chain_nonce: u64) {
        let mut nonces = self.nonces.lock().unwrap();
        let current = nonces.entry(address.to_string()).or_insert(0);
        if on_chain_nonce > *current {
            *current = on_chain_nonce;
        }
    }
}

impl Default for NonceManager {
    fn default() -> Self {
        Self::new()
    }
}
