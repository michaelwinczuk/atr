//! Wallet key generator for ATR setup
//!
//! Generates an EVM private key (and optionally a Solana keypair) for use with ATR.
//! Run with: cargo run -p atr-server --bin keygen

fn main() {
    println!("===========================================");
    println!("  Agent Transaction Router — Key Generator");
    println!("===========================================\n");

    // Generate EVM (Base) key
    let evm_key: [u8; 32] = rand::random();
    let evm_hex = hex::encode(evm_key);

    println!("--- Base (EVM) ---");
    println!("Private key: 0x{}", evm_hex);
    println!("Set in .env:  BASE_PRIVATE_KEY=0x{}", evm_hex);
    println!();

    // Generate Solana keypair
    #[cfg(feature = "solana")]
    {
        let secret: [u8; 32] = rand::random();
        let keypair_bytes: Vec<u8> = secret.to_vec();
        let bs58_key = bs58::encode(&keypair_bytes).into_string();

        println!("--- Solana ---");
        println!("Private key (base58, 32-byte seed): {}", bs58_key);
        println!("Set in .env:  SOLANA_PRIVATE_KEY={}", bs58_key);
        println!();
    }

    println!("IMPORTANT:");
    println!("  1. Save these keys securely — they cannot be recovered");
    println!("  2. Fund the wallets before submitting transactions:");
    println!("     - Base Mainnet: bridge ETH via https://bridge.base.org");
    println!("     - Base Sepolia: https://www.alchemy.com/faucets/base-sepolia");
    #[cfg(feature = "solana")]
    println!("     - Solana devnet: https://faucet.solana.com");
    println!("  3. Copy values to your .env file");
    println!("  4. NEVER commit .env to git");
}
