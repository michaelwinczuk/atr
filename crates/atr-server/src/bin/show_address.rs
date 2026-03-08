//! Show wallet addresses from .env private keys
//!
//! Run with: cargo run -p atr-server --bin show-address

use alloy::signers::local::PrivateKeySigner;
use solana_sdk::signer::Signer;

fn main() {
    dotenvy::dotenv().ok();

    println!("=== ATR Wallet Addresses ===\n");

    // Base (EVM)
    if let Ok(key) = std::env::var("BASE_PRIVATE_KEY") {
        match key.parse::<PrivateKeySigner>() {
            Ok(signer) => {
                println!("Base (EVM):");
                println!("  Address: {}", signer.address());
                println!("  Send Base Sepolia ETH to this address in MetaMask\n");
            }
            Err(e) => println!("Base: Failed to parse key: {}\n", e),
        }
    } else {
        println!("Base: BASE_PRIVATE_KEY not set in .env\n");
    }

    // Solana
    if let Ok(key) = std::env::var("SOLANA_PRIVATE_KEY") {
        match bs58::decode(&key).into_vec() {
            Ok(bytes) => {
                if bytes.len() == 32 {
                    match solana_sdk::signer::keypair::keypair_from_seed(&bytes) {
                        Ok(kp) => {
                            println!("Solana:");
                            println!("  Address: {}", kp.pubkey());
                            println!("  Fund on devnet: solana airdrop 2 {} --url devnet\n", kp.pubkey());
                        }
                        Err(e) => println!("Solana: Failed to derive keypair: {}\n", e),
                    }
                } else {
                    println!("Solana: Expected 32-byte seed, got {} bytes\n", bytes.len());
                }
            }
            Err(e) => println!("Solana: Failed to decode base58: {}\n", e),
        }
    } else {
        println!("Solana: SOLANA_PRIVATE_KEY not set in .env\n");
    }
}
