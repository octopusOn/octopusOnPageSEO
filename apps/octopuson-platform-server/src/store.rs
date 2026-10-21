//! The server's own database: accounts that signed in, their sessions, and when each account's
//! scheduled work is due next.
//!
//! A session is known by the SHA-256 of its cookie value only, so the database never holds
//! something a browser could present.

use std::path::Path;
use std::sync::{Mutex, PoisonError};

use duckdb::{Connection, params};
use sha2::{Digest, Sha256};

/// Database file name in the data folder.
pub const DATABASE: &str = "server.duckdb";

/// Errors of the store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database failed.
    #[error("database error: {0}")]
    Database(#[from] duckdb::Error),
}

/// A signed-in session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// The account.
    pub user_id: String,
    /// E-mail of the account when it signed in.
    pub email: Option<String>,
    /// Token pages send with every change, against cross-site requests.
    pub csrf: String,
}

/// A personal access token as its owner sees it (never the token itself).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AccessToken {
    /// Public id (the start of the token's hash).
    pub id: String,
    /// What the owner called it.
    pub name: String,
    /// Unix seconds.
    pub created_at: i64,
    /// Unix seconds.
    pub last_used_at: Option<i64>,
}

/// The server's database.
pub struct ServerStore {
    connection: Mutex<Connection>,
}

impl std::fmt::Debug for ServerStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerStore").finish_non_exhaustive()
    }
}

/// The stored form of a session cookie value.
pub fn session_hash(cookie: &str) -> String {
    let digest = Sha256::digest(cookie.as_bytes());
    digest
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            use std::fmt::Write as _;
            let _ = write!(text, "{byte:02x}");
            text
        })
}

impl ServerStore {
    /// Opens (and creates) the database in `dir`.
    ///
    /// # Errors
    /// Database errors.
    pub fn open(dir: &Path) -> Result<Self, StoreError> {
        let connection = Connection::open(dir.join(DATABASE))?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
                id VARCHAR PRIMARY KEY,
                email VARCHAR,
                created_at BIGINT NOT NULL,
                last_login_at BIGINT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sessions (
                hash VARCHAR PRIMARY KEY,
                user_id VARCHAR NOT NULL,
                csrf VARCHAR NOT NULL,
                created_at BIGINT NOT NULL,
                expires_at BIGINT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS schedules (
                user_id VARCHAR PRIMARY KEY,
                next_due BIGINT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS access_tokens (
                hash VARCHAR PRIMARY KEY,
                id VARCHAR NOT NULL,
                user_id VARCHAR NOT NULL,
                name VARCHAR NOT NULL,
                created_at BIGINT NOT NULL,
                last_used_at BIGINT
            );
            CREATE TABLE IF NOT EXISTS report_shares (
                token_hash VARCHAR PRIMARY KEY,
                user_id VARCHAR NOT NULL,
                created_at BIGINT NOT NULL
            );",
        )?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Records a sign-in of an account at `now` (Unix seconds).
    ///
    /// # Errors
    /// Database errors.
    pub fn signed_in(
        &self,
        user_id: &str,
        email: Option<&str>,
        now: i64,
    ) -> Result<(), StoreError> {
        self.connection().execute(
            "INSERT INTO users (id, email, created_at, last_login_at) VALUES (?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET email = excluded.email, last_login_at = excluded.last_login_at",
            params![user_id, email, now, now],
        )?;
        Ok(())
    }

    /// Opens a session for the cookie value `cookie`, valid until `expires_at`.
    ///
    /// # Errors
    /// Database errors.
    pub fn open_session(
        &self,
        cookie: &str,
        user_id: &str,
        csrf: &str,
        now: i64,
        expires_at: i64,
    ) -> Result<(), StoreError> {
        self.connection().execute(
            "INSERT INTO sessions (hash, user_id, csrf, created_at, expires_at) VALUES (?, ?, ?, ?, ?)",
            params![session_hash(cookie), user_id, csrf, now, expires_at],
        )?;
        Ok(())
    }

    /// The session of a cookie value, while valid at `now`.
    ///
    /// # Errors
    /// Database errors.
    pub fn session(&self, cookie: &str, now: i64) -> Result<Option<Session>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT s.user_id, u.email, s.csrf FROM sessions s LEFT JOIN users u ON u.id = s.user_id
             WHERE s.hash = ? AND s.expires_at > ?",
        )?;
        let mut rows = statement.query_map(params![session_hash(cookie), now], |row| {
            Ok(Session {
                user_id: row.get(0)?,
                email: row.get(1)?,
                csrf: row.get(2)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    /// Closes a session.
    ///
    /// # Errors
    /// Database errors.
    pub fn close_session(&self, cookie: &str) -> Result<(), StoreError> {
        self.connection().execute(
            "DELETE FROM sessions WHERE hash = ?",
            params![session_hash(cookie)],
        )?;
        Ok(())
    }

    /// Closes every session of an account (its token was revoked).
    ///
    /// # Errors
    /// Database errors.
    pub fn close_sessions_of(&self, user_id: &str) -> Result<(), StoreError> {
        self.connection()
            .execute("DELETE FROM sessions WHERE user_id = ?", params![user_id])?;
        Ok(())
    }

    /// Notes when an account's scheduled work is due next (Unix seconds); `None` forgets it.
    ///
    /// # Errors
    /// Database errors.
    pub fn set_next_due(&self, user_id: &str, next_due: Option<i64>) -> Result<(), StoreError> {
        let connection = self.connection();
        match next_due {
            Some(at) => connection.execute(
                "INSERT INTO schedules (user_id, next_due) VALUES (?, ?)
                 ON CONFLICT (user_id) DO UPDATE SET next_due = excluded.next_due",
                params![user_id, at],
            )?,
            None => {
                connection.execute("DELETE FROM schedules WHERE user_id = ?", params![user_id])?
            }
        };
        Ok(())
    }

    /// Accounts whose scheduled work is due at `now`, longest waiting first, at most `limit`.
    ///
    /// # Errors
    /// Database errors.
    pub fn due_accounts(&self, now: i64, limit: usize) -> Result<Vec<String>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT user_id FROM schedules WHERE next_due <= ? ORDER BY next_due, user_id LIMIT ?",
        )?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = statement.query_map(params![now, limit], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Notes whose report a share-link token opens (only its hash is kept).
    ///
    /// # Errors
    /// Database errors.
    pub fn add_report_share(&self, token: &str, user_id: &str, now: i64) -> Result<(), StoreError> {
        self.connection().execute(
            "INSERT INTO report_shares (token_hash, user_id, created_at) VALUES (?, ?, ?)
             ON CONFLICT (token_hash) DO UPDATE SET user_id = excluded.user_id",
            params![session_hash(token), user_id, now],
        )?;
        Ok(())
    }

    /// The account whose report a share-link token opens. The account's workspace still
    /// decides whether the token is valid (it may have been taken away).
    ///
    /// # Errors
    /// Database errors.
    pub fn report_share_owner(&self, token: &str) -> Result<Option<String>, StoreError> {
        let connection = self.connection();
        let mut statement =
            connection.prepare("SELECT user_id FROM report_shares WHERE token_hash = ?")?;
        let mut rows = statement.query_map(params![session_hash(token)], |row| row.get(0))?;
        Ok(rows.next().transpose()?)
    }

    /// Keeps a personal access token of an account (only its hash; its public id is the hash's
    /// first 12 characters).
    ///
    /// # Errors
    /// Database errors.
    pub fn add_access_token(
        &self,
        token: &str,
        user_id: &str,
        name: &str,
        now: i64,
    ) -> Result<AccessToken, StoreError> {
        let hash = session_hash(token);
        let id = hash[..12].to_owned();
        self.connection().execute(
            "INSERT INTO access_tokens (hash, id, user_id, name, created_at) VALUES (?, ?, ?, ?, ?)",
            params![hash, id, user_id, name, now],
        )?;
        Ok(AccessToken {
            id,
            name: name.to_owned(),
            created_at: now,
            last_used_at: None,
        })
    }

    /// An account's access tokens, newest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn access_tokens(&self, user_id: &str) -> Result<Vec<AccessToken>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, name, created_at, last_used_at FROM access_tokens WHERE user_id = ?
             ORDER BY created_at DESC, id",
        )?;
        let rows = statement.query_map(params![user_id], |row| {
            Ok(AccessToken {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                last_used_at: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Revokes an account's access token by its public id; whether one went.
    ///
    /// # Errors
    /// Database errors.
    pub fn revoke_access_token(&self, user_id: &str, id: &str) -> Result<bool, StoreError> {
        Ok(self.connection().execute(
            "DELETE FROM access_tokens WHERE user_id = ? AND id = ?",
            params![user_id, id],
        )? > 0)
    }

    /// The account an access token belongs to, noting its use at `now`.
    ///
    /// # Errors
    /// Database errors.
    pub fn access_token_user(&self, token: &str, now: i64) -> Result<Option<String>, StoreError> {
        let connection = self.connection();
        let hash = session_hash(token);
        let user: Option<String> = {
            let mut statement =
                connection.prepare("SELECT user_id FROM access_tokens WHERE hash = ?")?;
            let mut rows = statement.query_map(params![hash], |row| row.get(0))?;
            rows.next().transpose()?
        };
        if user.is_some() {
            connection.execute(
                "UPDATE access_tokens SET last_used_at = ? WHERE hash = ?",
                params![now, hash],
            )?;
        }
        Ok(user)
    }

    /// Removes sessions expired at `now`; how many went.
    ///
    /// # Errors
    /// Database errors.
    pub fn drop_expired(&self, now: i64) -> Result<usize, StoreError> {
        Ok(self
            .connection()
            .execute("DELETE FROM sessions WHERE expires_at <= ?", params![now])?)
    }
}
