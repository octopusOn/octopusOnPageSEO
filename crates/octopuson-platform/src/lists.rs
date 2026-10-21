//! Keyword lists: keywords kept for a project, each with its numbers when it was added.

use duckdb::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{PlatformStore, PlatformStoreError};

/// Most keywords in one list.
pub const MAX_LIST_KEYWORDS: usize = 10_000;

/// Longest list name.
const MAX_LIST_NAME: usize = 80;

/// A keyword list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeywordListRow {
    /// List id.
    pub id: i64,
    /// The project it belongs to.
    pub project_id: i64,
    /// Name.
    pub name: String,
    /// Keywords in it.
    pub keywords: u64,
    /// RFC 3339.
    pub created_at: String,
}

/// A keyword of a list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListKeyword {
    /// The keyword.
    pub keyword: String,
    /// Its numbers when it was added (volume, difficulty …), as the explorer had them.
    pub data: Value,
    /// RFC 3339.
    pub added_at: String,
}

fn list_name(name: &str) -> Result<String, PlatformStoreError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_LIST_NAME {
        return Err(PlatformStoreError::Invalid(
            "name: a list name of 1–80 characters".to_owned(),
        ));
    }
    Ok(name.to_owned())
}

impl PlatformStore {
    /// The lists of a project, by name, with their sizes.
    ///
    /// # Errors
    /// Database errors.
    pub fn keyword_lists(
        &self,
        project_id: i64,
    ) -> Result<Vec<KeywordListRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT l.id, l.project_id, l.name, count(k.keyword)::UBIGINT, l.created_at
             FROM keyword_lists l LEFT JOIN list_keywords k ON k.list_id = l.id
             WHERE l.project_id = ? GROUP BY l.id, l.project_id, l.name, l.created_at
             ORDER BY lower(l.name), l.id",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok(KeywordListRow {
                id: row.get(0)?,
                project_id: row.get(1)?,
                name: row.get(2)?,
                keywords: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// One list.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn keyword_list(&self, id: i64) -> Result<KeywordListRow, PlatformStoreError> {
        let project_id: Option<i64> = self
            .connection
            .query_row(
                "SELECT project_id FROM keyword_lists WHERE id = ?",
                params![id],
                |row| row.get(0),
            )
            .ok();
        let project_id =
            project_id.ok_or_else(|| PlatformStoreError::NotFound(format!("keyword list {id}")))?;
        self.keyword_lists(project_id)?
            .into_iter()
            .find(|list| list.id == id)
            .ok_or_else(|| PlatformStoreError::NotFound(format!("keyword list {id}")))
    }

    /// Makes a list in a project at `now` (RFC 3339).
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown project, [`PlatformStoreError::Invalid`]
    /// for an unusable name, or database errors.
    pub fn create_keyword_list(
        &self,
        project_id: i64,
        name: &str,
        now: &str,
    ) -> Result<KeywordListRow, PlatformStoreError> {
        self.project(project_id)?;
        let id: i64 = self.connection.query_row(
            "INSERT INTO keyword_lists (project_id, name, created_at) VALUES (?, ?, ?) RETURNING id",
            params![project_id, list_name(name)?, now],
            |row| row.get(0),
        )?;
        self.keyword_list(id)
    }

    /// Renames a list.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`], [`PlatformStoreError::Invalid`] or database errors.
    pub fn rename_keyword_list(
        &self,
        id: i64,
        name: &str,
    ) -> Result<KeywordListRow, PlatformStoreError> {
        let changed = self.connection.execute(
            "UPDATE keyword_lists SET name = ? WHERE id = ?",
            params![list_name(name)?, id],
        )?;
        if changed == 0 {
            return Err(PlatformStoreError::NotFound(format!("keyword list {id}")));
        }
        self.keyword_list(id)
    }

    /// Removes a list and its keywords.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn delete_keyword_list(&self, id: i64) -> Result<(), PlatformStoreError> {
        self.connection
            .execute("DELETE FROM list_keywords WHERE list_id = ?", params![id])?;
        let removed = self
            .connection
            .execute("DELETE FROM keyword_lists WHERE id = ?", params![id])?;
        if removed == 0 {
            return Err(PlatformStoreError::NotFound(format!("keyword list {id}")));
        }
        Ok(())
    }

    /// The keywords of a list, in the order they were added.
    ///
    /// # Errors
    /// Database errors, or stored numbers that are not JSON.
    pub fn list_keywords(&self, id: i64) -> Result<Vec<ListKeyword>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT keyword, data, added_at FROM list_keywords WHERE list_id = ? ORDER BY added_at, keyword",
        )?;
        let rows = statement.query_map(params![id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (keyword, data, added_at) = row?;
            Ok(ListKeyword {
                keyword,
                data: serde_json::from_str(&data)
                    .map_err(|error| PlatformStoreError::Corrupt(error.to_string()))?,
                added_at,
            })
        })
        .collect()
    }

    /// Adds keywords with their numbers at `now`; a keyword already in the list gets the new
    /// numbers. Returns how many keywords the list holds.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown list, [`PlatformStoreError::Invalid`]
    /// when the list would pass [`MAX_LIST_KEYWORDS`], or database errors.
    pub fn add_to_keyword_list(
        &self,
        id: i64,
        keywords: &[(String, Value)],
        now: &str,
    ) -> Result<u64, PlatformStoreError> {
        let list = self.keyword_list(id)?;
        let known: std::collections::HashSet<String> = self
            .list_keywords(id)?
            .into_iter()
            .map(|entry| entry.keyword)
            .collect();
        let new = keywords
            .iter()
            .filter(|(keyword, _)| !known.contains(keyword.trim()))
            .count();
        if usize::try_from(list.keywords).unwrap_or(usize::MAX) + new > MAX_LIST_KEYWORDS {
            return Err(PlatformStoreError::Invalid(format!(
                "keywords: a list holds at most {MAX_LIST_KEYWORDS} keywords"
            )));
        }
        let transaction = self.connection.unchecked_transaction()?;
        for (keyword, data) in keywords {
            let keyword = keyword.trim();
            if keyword.is_empty() {
                continue;
            }
            transaction.execute(
                "INSERT INTO list_keywords (list_id, keyword, data, added_at) VALUES (?, ?, ?, ?)
                 ON CONFLICT (list_id, keyword) DO UPDATE SET data = excluded.data",
                params![id, keyword, data.to_string(), now],
            )?;
        }
        transaction.commit()?;
        Ok(self.keyword_list(id)?.keywords)
    }

    /// Removes keywords from a list; returns how many keywords it still holds.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown list, or database errors.
    pub fn remove_from_keyword_list(
        &self,
        id: i64,
        keywords: &[String],
    ) -> Result<u64, PlatformStoreError> {
        self.keyword_list(id)?;
        for keyword in keywords {
            self.connection.execute(
                "DELETE FROM list_keywords WHERE list_id = ? AND keyword = ?",
                params![id, keyword.trim()],
            )?;
        }
        Ok(self.keyword_list(id)?.keywords)
    }
}
