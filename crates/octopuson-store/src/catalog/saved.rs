//! Saved queries: named query texts shared by every site of the catalog.

use duckdb::{OptionalExt, Row, params};
use octopuson_core::Timestamp;

use super::Catalog;
use super::model::SavedQuery;
use crate::error::StoreError;

const COLUMNS: &str = "id, name, description, query, epoch_us(created_at), epoch_us(updated_at)";

impl Catalog {
    /// All saved queries ordered by name.
    ///
    /// # Errors
    /// Database errors.
    pub fn saved_queries(&self) -> Result<Vec<SavedQuery>, StoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM saved_queries ORDER BY lower(name), name"
        ))?;
        let rows = statement.query_map([], saved_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Looks a saved query up by exact name.
    ///
    /// # Errors
    /// [`StoreError::SavedQueryNotFound`] if absent.
    pub fn saved_query(&self, name: &str) -> Result<SavedQuery, StoreError> {
        self.connection
            .query_row(
                &format!("SELECT {COLUMNS} FROM saved_queries WHERE name = ?"),
                params![name],
                saved_from_row,
            )
            .optional()?
            .ok_or_else(|| StoreError::SavedQueryNotFound(name.to_owned()))
    }

    /// Saves a query under `name`. An existing query of that name is replaced only when
    /// `overwrite` is set (its creation time is kept).
    ///
    /// # Errors
    /// [`StoreError::DuplicateSavedQuery`] when the name is taken and `overwrite` is unset.
    pub fn save_query(
        &self,
        name: &str,
        description: &str,
        query: &str,
        overwrite: bool,
    ) -> Result<SavedQuery, StoreError> {
        let now = Timestamp::now().as_unix_micros();
        match self.saved_query(name) {
            Ok(_) if !overwrite => Err(StoreError::DuplicateSavedQuery(name.to_owned())),
            Ok(existing) => {
                self.connection.execute(
                    "UPDATE saved_queries SET description = ?, query = ?,
                     updated_at = make_timestamp(?) WHERE id = ?",
                    params![description, query, now, existing.id],
                )?;
                self.saved_query(name)
            }
            Err(StoreError::SavedQueryNotFound(_)) => {
                self.connection.execute(
                    "INSERT INTO saved_queries (name, description, query, created_at, updated_at)
                     VALUES (?, ?, ?, make_timestamp(?), make_timestamp(?))",
                    params![name, description, query, now, now],
                )?;
                self.saved_query(name)
            }
            Err(other) => Err(other),
        }
    }

    /// Deletes a saved query.
    ///
    /// # Errors
    /// [`StoreError::SavedQueryNotFound`] if absent.
    pub fn remove_saved_query(&self, name: &str) -> Result<(), StoreError> {
        let removed = self
            .connection
            .execute("DELETE FROM saved_queries WHERE name = ?", params![name])?;
        if removed == 0 {
            return Err(StoreError::SavedQueryNotFound(name.to_owned()));
        }
        Ok(())
    }
}

fn saved_from_row(row: &Row<'_>) -> duckdb::Result<SavedQuery> {
    Ok(SavedQuery {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        query: row.get(3)?,
        created_at: Timestamp::from_unix_micros(row.get(4)?),
        updated_at: Timestamp::from_unix_micros(row.get(5)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::DataDir;

    #[test]
    fn saves_lists_overwrites_and_removes() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(&DataDir::new(dir.path())).unwrap();
        let first = catalog
            .save_query(
                "Googlebot 404",
                "weekly check",
                "bot:googlebot* status:404",
                false,
            )
            .unwrap();
        catalog
            .save_query("a report", "", "group:url", false)
            .unwrap();
        let names: Vec<String> = catalog
            .saved_queries()
            .unwrap()
            .into_iter()
            .map(|q| q.name)
            .collect();
        assert_eq!(names, ["a report", "Googlebot 404"]);

        assert!(matches!(
            catalog.save_query("Googlebot 404", "", "status:410", false),
            Err(StoreError::DuplicateSavedQuery(_))
        ));
        let replaced = catalog
            .save_query("Googlebot 404", "updated", "status:410", true)
            .unwrap();
        assert_eq!(
            (replaced.id, replaced.query.as_str()),
            (first.id, "status:410")
        );
        assert_eq!(replaced.created_at, first.created_at);
        assert!(replaced.updated_at >= first.updated_at);

        catalog.remove_saved_query("a report").unwrap();
        assert!(matches!(
            catalog.remove_saved_query("a report"),
            Err(StoreError::SavedQueryNotFound(_))
        ));
        assert_eq!(catalog.saved_queries().unwrap().len(), 1);
    }
}
