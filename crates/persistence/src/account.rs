//! Account and session repository.
//!
//! Handles account creation, authentication, and session management.

use chrono::{Duration, Utc};
use sqlx::PgPool;
use tracing::{debug, info};
use uuid::Uuid;

use crate::error::{PersistenceError, Result};
use crate::models::{Account, NewAccount, Session};

/// Account repository for database operations.
pub struct AccountRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> AccountRepo<'a> {
    /// Create a new account repository.
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Create a new account.
    pub async fn create(&self, new_account: NewAccount) -> Result<Account> {
        // Check if username already exists
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM accounts WHERE username = $1"
        )
        .bind(&new_account.username)
        .fetch_one(self.pool)
        .await?;

        if existing > 0 {
            return Err(PersistenceError::UsernameExists(new_account.username));
        }

        let account_id = Uuid::new_v4();
        let now = Utc::now();

        let account = sqlx::query_as::<_, Account>(
            r#"
            INSERT INTO accounts (account_id, username, password_hash, created_at)
            VALUES ($1, $2, $3, $4)
            RETURNING account_id, username, password_hash, created_at
            "#,
        )
        .bind(account_id)
        .bind(&new_account.username)
        .bind(&new_account.password_hash)
        .bind(now)
        .fetch_one(self.pool)
        .await?;

        info!("Created account: {} ({})", account.username, account.account_id);
        Ok(account)
    }

    /// Get account by username.
    pub async fn get_by_username(&self, username: &str) -> Result<Account> {
        sqlx::query_as::<_, Account>(
            "SELECT account_id, username, password_hash, created_at FROM accounts WHERE username = $1",
        )
        .bind(username)
        .fetch_optional(self.pool)
        .await?
        .ok_or_else(|| PersistenceError::AccountNotFound(username.to_string()))
    }

    /// Get account by ID.
    pub async fn get_by_id(&self, account_id: Uuid) -> Result<Account> {
        sqlx::query_as::<_, Account>(
            "SELECT account_id, username, password_hash, created_at FROM accounts WHERE account_id = $1",
        )
        .bind(account_id)
        .fetch_optional(self.pool)
        .await?
        .ok_or_else(|| PersistenceError::AccountNotFound(account_id.to_string()))
    }

    /// Get or create account for dev auth (no password).
    ///
    /// In development mode, accounts are created automatically on first login.
    pub async fn get_or_create_dev(&self, username: &str) -> Result<Account> {
        match self.get_by_username(username).await {
            Ok(account) => Ok(account),
            Err(PersistenceError::AccountNotFound(_)) => {
                debug!("Creating dev account for: {}", username);
                self.create(NewAccount {
                    username: username.to_string(),
                    password_hash: None,
                })
                .await
            }
            Err(e) => Err(e),
        }
    }

    /// Create a new session for an account.
    pub async fn create_session(&self, account_id: Uuid) -> Result<Session> {
        let session_token = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::hours(24);

        let session = sqlx::query_as::<_, Session>(
            r#"
            INSERT INTO sessions (session_token, account_id, expires_at)
            VALUES ($1, $2, $3)
            RETURNING session_token, account_id, expires_at
            "#,
        )
        .bind(&session_token)
        .bind(account_id)
        .bind(expires_at)
        .fetch_one(self.pool)
        .await?;

        debug!("Created session for account {}", account_id);
        Ok(session)
    }

    /// Validate and get session.
    pub async fn get_session(&self, session_token: &str) -> Result<Session> {
        let session = sqlx::query_as::<_, Session>(
            "SELECT session_token, account_id, expires_at FROM sessions WHERE session_token = $1",
        )
        .bind(session_token)
        .fetch_optional(self.pool)
        .await?
        .ok_or(PersistenceError::SessionNotFound)?;

        // Check if expired
        if session.expires_at < Utc::now() {
            // Delete expired session
            let _ = self.delete_session(session_token).await;
            return Err(PersistenceError::SessionNotFound);
        }

        Ok(session)
    }

    /// Delete a session (logout).
    pub async fn delete_session(&self, session_token: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE session_token = $1")
            .bind(session_token)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Delete all sessions for an account.
    pub async fn delete_all_sessions(&self, account_id: Uuid) -> Result<u64> {
        let result = sqlx::query("DELETE FROM sessions WHERE account_id = $1")
            .bind(account_id)
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Clean up expired sessions.
    pub async fn cleanup_expired_sessions(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at < $1")
            .bind(Utc::now())
            .execute(self.pool)
            .await?;
        
        if result.rows_affected() > 0 {
            info!("Cleaned up {} expired sessions", result.rows_affected());
        }
        
        Ok(result.rows_affected())
    }
}
