//! SQLite implementation of SystemStateRepository (SYS-01, D-22).

use crate::error::M31AError;
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Row, SqlitePool};

/// Trait for canonical system state key-value persistence.
#[async_trait]
pub trait SystemStateRepository: Send + Sync {
    /// Retrieve a value by key.
    async fn get(&self, key: &str) -> Result<Option<String>, M31AError>;

    /// Insert or update a key-value pair.
    async fn set(&self, key: &str, value: &str) -> Result<(), M31AError>;

    /// Delete a key-value pair.
    async fn delete(&self, key: &str) -> Result<(), M31AError>;

    /// List all key-value pairs.
    async fn list_all(&self) -> Result<Vec<(String, String)>, M31AError>;
}

/// SQLite concrete repository for system state.
#[derive(Debug, Clone)]
pub struct SqliteSystemStateRepository {
    pool: SqlitePool,
}

impl SqliteSystemStateRepository {
    /// Create a new `SqliteSystemStateRepository`.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the underlying connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Persist onboarding sentinel and init_state atomically or sequentially.
    pub async fn record_onboarding(
        &self,
        sentinel_json: &str,
        init_state_str: &str,
    ) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO system_state (key, value, updated_at)
            VALUES ('onboarding_sentinel', ?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
            "#,
        )
        .bind(sentinel_json)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO system_state (key, value, updated_at)
            VALUES ('init_state', ?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
            "#,
        )
        .bind(init_state_str)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    /// Retrieve a value by key.
    pub async fn get(&self, key: &str) -> Result<Option<String>, M31AError> {
        <Self as SystemStateRepository>::get(self, key).await
    }

    /// Insert or update a key-value pair.
    pub async fn set(&self, key: &str, value: &str) -> Result<(), M31AError> {
        <Self as SystemStateRepository>::set(self, key, value).await
    }

    /// Delete a key-value pair.
    pub async fn delete(&self, key: &str) -> Result<(), M31AError> {
        <Self as SystemStateRepository>::delete(self, key).await
    }

    /// List all key-value pairs.
    pub async fn list_all(&self) -> Result<Vec<(String, String)>, M31AError> {
        <Self as SystemStateRepository>::list_all(self).await
    }
}

#[async_trait]
impl SystemStateRepository for SqliteSystemStateRepository {
    async fn get(&self, key: &str) -> Result<Option<String>, M31AError> {
        let maybe_row = sqlx::query("SELECT value FROM system_state WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?;

        match maybe_row {
            Some(row) => {
                let val: String = row
                    .try_get("value")
                    .map_err(|e| M31AError::persistence(e.to_string()))?;
                Ok(Some(val))
            }
            None => Ok(None),
        }
    }

    async fn set(&self, key: &str, value: &str) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO system_state (key, value, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
            "#,
        )
        .bind(key)
        .bind(value)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), M31AError> {
        sqlx::query("DELETE FROM system_state WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<(String, String)>, M31AError> {
        let rows = sqlx::query("SELECT key, value FROM system_state ORDER BY key ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?;

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let key: String = row
                .try_get("key")
                .map_err(|e| M31AError::persistence(e.to_string()))?;
            let val: String = row
                .try_get("value")
                .map_err(|e| M31AError::persistence(e.to_string()))?;
            result.push((key, val));
        }

        Ok(result)
    }
}
