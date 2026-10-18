//! Cached facts about domains and imported datasets.

use std::path::Path;

use duckdb::{OptionalExt, params};
use serde::{Deserialize, Serialize};

use crate::{SerpStore, SerpStoreError};

/// One cached fact about a domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainFact {
    /// Registrable domain.
    pub domain: String,
    /// What it is (`age`, `pagerank`, `backlinks`).
    pub signal: String,
    /// The value as JSON, `None` when the lookup failed.
    pub value: Option<String>,
    /// Where it came from (`rdap`, `whois`, `open-pagerank`, ...).
    pub source: Option<String>,
    /// When it was looked up, RFC 3339.
    pub fetched_at: String,
    /// When it should be looked up again, RFC 3339.
    pub expires_at: String,
    /// Why the lookup failed.
    pub error: Option<String>,
}

/// An imported dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dataset {
    /// Name (`tranco`).
    pub name: String,
    /// Version (the Tranco list id).
    pub version: Option<String>,
    /// When it was imported.
    pub imported_at: String,
    /// Rows.
    pub rows: u64,
}

impl SerpStore {
    /// A cached fact, expired or not.
    ///
    /// # Errors
    /// Database errors.
    pub fn domain_fact(
        &self,
        domain: &str,
        signal: &str,
    ) -> Result<Option<DomainFact>, SerpStoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT domain, signal, value, source, fetched_at, expires_at, error
                 FROM domain_facts WHERE domain = ? AND signal = ?",
                params![domain, signal],
                |row| {
                    Ok(DomainFact {
                        domain: row.get(0)?,
                        signal: row.get(1)?,
                        value: row.get(2)?,
                        source: row.get(3)?,
                        fetched_at: row.get(4)?,
                        expires_at: row.get(5)?,
                        error: row.get(6)?,
                    })
                },
            )
            .optional()?)
    }

    /// Caches a fact, replacing the previous one.
    ///
    /// # Errors
    /// Database errors.
    pub fn put_domain_fact(&self, fact: &DomainFact) -> Result<(), SerpStoreError> {
        self.connection.execute(
            "INSERT OR REPLACE INTO domain_facts VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                fact.domain,
                fact.signal,
                fact.value,
                fact.source,
                fact.fetched_at,
                fact.expires_at,
                fact.error
            ],
        )?;
        Ok(())
    }

    /// Replaces the Tranco ranks with a `rank,domain` CSV file.
    ///
    /// # Errors
    /// Database errors, e.g. for a malformed file.
    pub fn import_tranco(
        &mut self,
        csv: &Path,
        version: Option<&str>,
        imported_at: &str,
    ) -> Result<u64, SerpStoreError> {
        let path = csv.to_string_lossy().replace('\'', "''");
        let transaction = self.connection.transaction()?;
        transaction.execute_batch(&format!(
            "DELETE FROM tranco;
             INSERT INTO tranco
             SELECT lower(column1), min(column0)::INTEGER FROM read_csv('{path}', header = false,
                 columns = {{'column0': 'INTEGER', 'column1': 'VARCHAR'}})
             WHERE column1 IS NOT NULL GROUP BY lower(column1);"
        ))?;
        let rows: u64 =
            transaction.query_row("SELECT count(*)::UBIGINT FROM tranco", [], |row| row.get(0))?;
        transaction.execute(
            "INSERT OR REPLACE INTO datasets VALUES ('tranco', ?, ?, ?)",
            params![version, imported_at, rows],
        )?;
        transaction.commit()?;
        Ok(rows)
    }

    /// Tranco rank of a registrable domain.
    ///
    /// # Errors
    /// Database errors.
    pub fn tranco_rank(&self, domain: &str) -> Result<Option<u32>, SerpStoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT rank FROM tranco WHERE domain = ?",
                params![domain.to_ascii_lowercase()],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// An imported dataset.
    ///
    /// # Errors
    /// Database errors.
    pub fn dataset(&self, name: &str) -> Result<Option<Dataset>, SerpStoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT name, version, imported_at, rows::UBIGINT FROM datasets WHERE name = ?",
                params![name],
                |row| {
                    Ok(Dataset {
                        name: row.get(0)?,
                        version: row.get(1)?,
                        imported_at: row.get(2)?,
                        rows: row.get(3)?,
                    })
                },
            )
            .optional()?)
    }
}
