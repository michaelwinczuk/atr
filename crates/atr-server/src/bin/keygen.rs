//! Wallet key generator for ATR setup
//!
//! Generates a Solana keypair and an EVM private key for use with ATR.
//! Run with: cargo run -p atr-server --bin keygen

fn main() {
    println!("===========================================");
    println!("  Agent Transaction Router — Key Generator");
    println!("===========================================\n");

    // Generate EVM (Base) key
    let evm_key: [u8; 32] = rand::random();
    let evm_hex = hex::encode(evm_key);

    // Derive EVM address using keccak256 of the public key
    // For simplicity, we just show the private key — the server derives the address
    println!("--- Base (EVM) ---");
    println!("Private key: 0x{}", evm_hex);
    println!("Set in .env:  BASE_PRIVATE_KEY=0x{}", evm_hex);
    println!();

    // Generate Solana keypair (64 bytes: 32 secret + 32 public)
    let secret: [u8; 32] = rand::random();
    // ed25519: public key derived from secret
    // For a proper keypair we need ed25519, but we can generate random bytes
    // and let the Solana SDK derive the keypair
    let keypair_bytes: Vec<u8> = secret.to_vec();
    let bs58_key = bs58::encode(&keypair_bytes).into_string();

    println!("--- Solana ---");
    println!("Private key (base58, 32-byte seed): {}", bs58_key);
    println!("Set in .env:  SOLANA_PRIVATE_KEY={}", bs58_key);
    println!();

    println!("IMPORTANT:");
    println!("  1. Save these keys securely — they cannot be recovered");
    println!("  2. Fund the wallets before submitting transactions:");
    println!("     - Solana devnet: https://faucet.solana.com");
    println!("     - Base Sepolia:  https://www.alchemy.com/faucets/base-sepolia");
    println!("  3. Copy values to your .env file");
    println!("  4. NEVER commit .env to git");
}
