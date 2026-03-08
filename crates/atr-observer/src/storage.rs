//! SQLite persistent storage for transaction records and API keys

use atr_core::chain::Chain;
use atr_core::transaction::{TransactionRecord, TransactionStatus};
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use tracing::info;
use uuid::Uuid;

/// Persistent storage backed by SQLite
#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
}

impl Storage {
    /// Create a new storage instance and run migrations
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        let storage = Self { pool };
        storage.run_migrations().await?;
        info!("SQLite storage initialized");
        Ok(storage)
    }

    /// Run schema migrations
    async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS transactions (
                id TEXT PRIMARY KEY,
                chain TEXT NOT NULL,
                status TEXT NOT NULL,
                tx_hash TEXT,
                block_number INTEGER,
                units_used INTEGER,
                fee_paid INTEGER,
                error TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                finalized_at TEXT
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS api_keys (
                key TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL,
                rate_limit INTEGER NOT NULL DEFAULT 100,
                active INTEGER NOT NULL DEFAULT 1
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS idempotency_keys (
                key TEXT PRIMARY KEY,
                intent_id TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        // Index for fast status lookups
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status)",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // --- Transaction Operations ---

    /// Save or update a transaction record
    pub async fn save_transaction(&self, record: &TransactionRecord) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO transactions
             (id, chain, status, tx_hash, block_number, units_used, fee_paid, error, retry_count, created_at, updated_at, finalized_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(record.id.to_string())
        .bind(record.chain.to_string())
        .bind(status_to_str(record.status))
        .bind(&record.tx_hash)
        .bind(record.block_number.map(|n| n as i64))
        .bind(record.units_used.map(|n| n as i64))
        .bind(record.fee_paid.map(|n| n as i64))
        .bind(&record.error)
        .bind(record.retry_count as i32)
        .bind(record.created_at.to_rfc3339())
        .bind(record.updated_at.to_rfc3339())
        .bind(record.finalized_at.map(|t| t.to_rfc3339()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update transaction status
    pub async fn update_transaction_status(
        &self,
        id: Uuid,
        status: TransactionStatus,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let finalized = if matches!(
            status,
            TransactionStatus::Finalized | TransactionStatus::Failed
        ) {
            Some(now.clone())
        } else {
            None
        };

        sqlx::query(
            "UPDATE transactions SET status = ?, updated_at = ?, finalized_at = COALESCE(?, finalized_at) WHERE id = ?",
        )
        .bind(status_to_str(status))
        .bind(&now)
        .bind(finalized)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Set tx_hash for a transaction
    pub async fn set_tx_hash(&self, id: Uuid, tx_hash: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE transactions SET tx_hash = ?, updated_at = ? WHERE id = ?")
            .bind(tx_hash)
            .bind(Utc::now().to_rfc3339())
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get a transaction by ID
    pub async fn get_transaction(&self, id: Uuid) -> Result<Option<TransactionRecord>, sqlx::Error> {
        let row = sqlx::query("SELECT * FROM transactions WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_record(&r)))
    }

    /// Get all non-terminal transactions (for confirmation polling)
    pub async fn get_pending_transactions(&self) -> Result<Vec<TransactionRecord>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT * FROM transactions WHERE status NOT IN ('finalized', 'failed', 'dropped', 'simulation_failed')",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(row_to_record).collect())
    }

    /// Update block number for a transaction
    pub async fn update_transaction_block(
        &self,
        id: Uuid,
        block_number: u64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE transactions SET block_number = ?, updated_at = ? WHERE id = ?")
            .bind(block_number as i64)
            .bind(Utc::now().to_rfc3339())
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update fee paid for a transaction
    pub async fn update_transaction_fee(
        &self,
        id: Uuid,
        fee_paid: u64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE transactions SET fee_paid = ?, updated_at = ? WHERE id = ?")
            .bind(fee_paid as i64)
            .bind(Utc::now().to_rfc3339())
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // --- API Key Operations ---

    /// Create a new API key
    pub async fn create_api_key(&self, name: &str) -> Result<String, sqlx::Error> {
        let key = format!("atr_{}", Uuid::new_v4().to_string().replace('-', ""));
        sqlx::query("INSERT INTO api_keys (key, name, created_at) VALUES (?, ?, ?)")
            .bind(&key)
            .bind(name)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(key)
    }

    /// Validate an API key — returns true if key exists and is active
    pub async fn validate_api_key(&self, key: &str) -> Result<bool, sqlx::Error> {
        let row = sqlx::query("SELECT active FROM api_keys WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| r.get::<bool, _>("active")).unwrap_or(false))
    }

    /// Revoke an API key
    pub async fn revoke_api_key(&self, key: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE api_keys SET active = 0 WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// List all API keys
    pub async fn list_api_keys(&self) -> Result<Vec<(String, String, bool)>, sqlx::Error> {
        let rows = sqlx::query("SELECT key, name, active FROM api_keys")
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .iter()
            .map(|r| {
                (
                    r.get::<String, _>("key"),
                    r.get::<String, _>("name"),
                    r.get::<bool, _>("active"),
                )
            })
            .collect())
    }

    // --- Idempotency Operations ---

    /// Check and set idempotency key. Returns existing intent_id if key already exists.
    pub async fn check_idempotency(
        &self,
        key: &str,
        intent_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let existing = sqlx::query("SELECT intent_id FROM idempotency_keys WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = existing {
            let existing_id: String = row.get("intent_id");
            Ok(Some(
                Uuid::parse_str(&existing_id).unwrap_or(intent_id),
            ))
        } else {
            sqlx::query("INSERT INTO idempotency_keys (key, intent_id, created_at) VALUES (?, ?, ?)")
                .bind(key)
                .bind(intent_id.to_string())
                .bind(Utc::now().to_rfc3339())
                .execute(&self.pool)
                .await?;
            Ok(None)
        }
    }
}

fn status_to_str(status: TransactionStatus) -> &'static str {
    match status {
        TransactionStatus::Pending => "pending",
        TransactionStatus::Simulating => "simulating",
        TransactionStatus::Simulated => "simulated",
        TransactionStatus::SimulationFailed => "simulation_failed",
        TransactionStatus::Submitted => "submitted",
        TransactionStatus::Confirmed => "confirmed",
        TransactionStatus::Finalized => "finalized",
        TransactionStatus::Failed => "failed",
        TransactionStatus::Dropped => "dropped",
        TransactionStatus::Retrying => "retrying",
    }
}

fn str_to_status(s: &str) -> TransactionStatus {
    match s {
        "pending" => TransactionStatus::Pending,
        "simulating" => TransactionStatus::Simulating,
        "simulated" => TransactionStatus::Simulated,
        "simulation_failed" => TransactionStatus::SimulationFailed,
        "submitted" => TransactionStatus::Submitted,
        "confirmed" => TransactionStatus::Confirmed,
        "finalized" => TransactionStatus::Finalized,
        "failed" => TransactionStatus::Failed,
        "dropped" => TransactionStatus::Dropped,
        "retrying" => TransactionStatus::Retrying,
        _ => TransactionStatus::Pending,
    }
}

fn str_to_chain(s: &str) -> Chain {
    match s {
        "solana" => Chain::Solana,
        "base" => Chain::Base,
        _ => Chain::Base,
    }
}

fn row_to_record(row: &sqlx::sqlite::SqliteRow) -> TransactionRecord {
    let id_str: String = row.get("id");
    let chain_str: String = row.get("chain");
    let status_str: String = row.get("status");
    let created_str: String = row.get("created_at");
    let updated_str: String = row.get("updated_at");
    let finalized_str: Option<String> = row.get("finalized_at");

    TransactionRecord {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
        chain: str_to_chain(&chain_str),
        status: str_to_status(&status_str),
        tx_hash: row.get("tx_hash"),
        block_number: row.get::<Option<i64>, _>("block_number").map(|n| n as u64),
        units_used: row.get::<Option<i64>, _>("units_used").map(|n| n as u64),
        fee_paid: row.get::<Option<i64>, _>("fee_paid").map(|n| n as u64),
        error: row.get("error"),
        retry_count: row.get::<i32, _>("retry_count") as u32,
        created_at: DateTime::parse_from_rfc3339(&created_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        updated_at: DateTime::parse_from_rfc3339(&updated_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        finalized_at: finalized_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        }),
    }
}
