//! Catalog database: clients, their sites and saved queries.

mod model;
mod saved;

pub use model::{Client, ClientId, NewSite, SavedQuery, Site, SiteId};

use std::sync::{Mutex, MutexGuard, PoisonError};

use duckdb::{Connection, OptionalExt, Row, params};
use seonexus_core::Timestamp;

use crate::db::{self, Migration};
use crate::error::StoreError;
use crate::paths::DataDir;
use crate::settings::StoreSettings;

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: "
        CREATE SEQUENCE client_id_seq START 1;
        CREATE TABLE clients (
            id BIGINT PRIMARY KEY DEFAULT nextval('client_id_seq'),
            name VARCHAR NOT NULL UNIQUE,
            created_at TIMESTAMP NOT NULL
        );
        CREATE SEQUENCE site_id_seq START 1;
        CREATE TABLE sites (
            id BIGINT PRIMARY KEY DEFAULT nextval('site_id_seq'),
            client_id BIGINT NOT NULL REFERENCES clients (id),
            name VARCHAR NOT NULL UNIQUE,
            format_spec VARCHAR NOT NULL,
            created_at TIMESTAMP NOT NULL
        );
    ",
    },
    Migration {
        version: 2,
        sql: "
        CREATE SEQUENCE saved_query_id_seq START 1;
        CREATE TABLE saved_queries (
            id BIGINT PRIMARY KEY DEFAULT nextval('saved_query_id_seq'),
            name VARCHAR NOT NULL UNIQUE,
            description VARCHAR NOT NULL,
            query VARCHAR NOT NULL,
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        );
    ",
    },
    Migration {
        version: 3,
        // Per-site settings as JSON, interpreted by the application layer.
        sql: "ALTER TABLE sites ADD COLUMN settings VARCHAR DEFAULT '{}';",
    },
];

const SITE_COLUMNS: &str = "s.id, s.client_id, c.name, s.name, s.format_spec, \
     epoch_us(s.created_at), coalesce(s.settings, '{}')";

/// Serializes catalog handles within the process. DuckDB must not open one file as two
/// independent database instances: they would not see each other's writes and could corrupt
/// the file. Catalog operations are short, so callers (e.g. parallel desktop commands) simply
/// take turns.
static CATALOG_GUARD: Mutex<()> = Mutex::new(());

/// Handle to the catalog database. At most one is open per process at a time; drop it as
/// soon as possible and never open a second one while holding it.
#[derive(Debug)]
pub struct Catalog {
    // Declared first so the connection closes before the guard is released.
    connection: Connection,
    _guard: MutexGuard<'static, ()>,
}

impl Catalog {
    /// Opens (creating and migrating if needed) the catalog of `data_dir`.
    ///
    /// # Errors
    /// Database and filesystem errors.
    pub fn open(data_dir: &DataDir) -> Result<Self, StoreError> {
        let guard = CATALOG_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
        let connection =
            db::open_migrated(&data_dir.catalog_db(), MIGRATIONS, StoreSettings::default())?;
        Ok(Self {
            connection,
            _guard: guard,
        })
    }

    /// Creates a client.
    ///
    /// # Errors
    /// [`StoreError::DuplicateClient`] if the name is taken.
    pub fn add_client(&self, name: &str) -> Result<Client, StoreError> {
        if self.client_by_name(name).is_ok() {
            return Err(StoreError::DuplicateClient(name.to_owned()));
        }
        let created_at = Timestamp::now();
        let id = self.connection.query_row(
            "INSERT INTO clients (name, created_at) VALUES (?, make_timestamp(?)) RETURNING id",
            params![name, created_at.as_unix_micros()],
            |row| row.get(0),
        )?;
        Ok(Client {
            id: ClientId::new(id),
            name: name.to_owned(),
            created_at,
        })
    }

    /// All clients ordered by name.
    ///
    /// # Errors
    /// Database errors.
    pub fn clients(&self) -> Result<Vec<Client>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name, epoch_us(created_at) FROM clients ORDER BY name")?;
        let rows = statement.query_map([], client_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Looks up a client by exact name.
    ///
    /// # Errors
    /// [`StoreError::ClientNotFound`] if absent.
    pub fn client_by_name(&self, name: &str) -> Result<Client, StoreError> {
        self.connection
            .query_row(
                "SELECT id, name, epoch_us(created_at) FROM clients WHERE name = ?",
                params![name],
                client_from_row,
            )
            .optional()?
            .ok_or_else(|| StoreError::ClientNotFound(name.to_owned()))
    }

    /// Creates a site under an existing client.
    ///
    /// # Errors
    /// [`StoreError::ClientNotFound`] or [`StoreError::DuplicateSite`].
    pub fn add_site(&self, new_site: NewSite<'_>) -> Result<Site, StoreError> {
        let client = self.client_by_name(new_site.client_name)?;
        if self.site_by_name(new_site.name).is_ok() {
            return Err(StoreError::DuplicateSite(new_site.name.to_owned()));
        }
        let created_at = Timestamp::now();
        let id = self.connection.query_row(
            "INSERT INTO sites (client_id, name, format_spec, created_at)
             VALUES (?, ?, ?, make_timestamp(?)) RETURNING id",
            params![
                client.id.get(),
                new_site.name,
                new_site.format_spec,
                created_at.as_unix_micros()
            ],
            |row| row.get(0),
        )?;
        Ok(Site {
            id: SiteId::new(id),
            client_id: client.id,
            client_name: client.name,
            name: new_site.name.to_owned(),
            format_spec: new_site.format_spec.to_owned(),
            created_at,
            settings: "{}".to_owned(),
        })
    }

    /// All sites ordered by client and site name.
    ///
    /// # Errors
    /// Database errors.
    pub fn sites(&self) -> Result<Vec<Site>, StoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {SITE_COLUMNS} FROM sites s JOIN clients c ON c.id = s.client_id
             ORDER BY c.name, s.name"
        ))?;
        let rows = statement.query_map([], site_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Changes the stored log format of a site; later imports use it.
    ///
    /// # Errors
    /// [`StoreError::SiteNotFound`] if absent.
    pub fn set_site_format(&self, id: SiteId, format_spec: &str) -> Result<(), StoreError> {
        let updated = self.connection.execute(
            "UPDATE sites SET format_spec = ? WHERE id = ?",
            params![format_spec, id.get()],
        )?;
        if updated == 0 {
            return Err(StoreError::SiteNotFound(id.to_string()));
        }
        Ok(())
    }

    /// Replaces a site's settings (serialized by the application layer).
    ///
    /// # Errors
    /// [`StoreError::SiteNotFound`] if absent.
    pub fn set_site_settings(&self, id: SiteId, settings: &str) -> Result<(), StoreError> {
        let updated = self.connection.execute(
            "UPDATE sites SET settings = ? WHERE id = ?",
            params![settings, id.get()],
        )?;
        if updated == 0 {
            return Err(StoreError::SiteNotFound(id.to_string()));
        }
        Ok(())
    }

    /// Deletes a site's catalog entry (its data directory is removed separately, see
    /// [`SiteStore::destroy`](crate::SiteStore::destroy)).
    ///
    /// # Errors
    /// [`StoreError::SiteNotFound`] if absent.
    pub fn remove_site(&self, id: SiteId) -> Result<(), StoreError> {
        let removed = self
            .connection
            .execute("DELETE FROM sites WHERE id = ?", params![id.get()])?;
        if removed == 0 {
            return Err(StoreError::SiteNotFound(id.to_string()));
        }
        Ok(())
    }

    /// Deletes a client that has no sites.
    ///
    /// # Errors
    /// [`StoreError::ClientNotFound`] or [`StoreError::ClientHasSites`].
    pub fn remove_client(&self, name: &str) -> Result<(), StoreError> {
        let client = self.client_by_name(name)?;
        let sites: i64 = self.connection.query_row(
            "SELECT count(*) FROM sites WHERE client_id = ?",
            params![client.id.get()],
            |row| row.get(0),
        )?;
        if sites > 0 {
            return Err(StoreError::ClientHasSites(client.name));
        }
        self.connection
            .execute("DELETE FROM clients WHERE id = ?", params![client.id.get()])?;
        Ok(())
    }

    /// Looks up a site by exact name.
    ///
    /// # Errors
    /// [`StoreError::SiteNotFound`] if absent.
    pub fn site_by_name(&self, name: &str) -> Result<Site, StoreError> {
        self.connection
            .query_row(
                &format!(
                    "SELECT {SITE_COLUMNS} FROM sites s JOIN clients c ON c.id = s.client_id
                     WHERE s.name = ?"
                ),
                params![name],
                site_from_row,
            )
            .optional()?
            .ok_or_else(|| StoreError::SiteNotFound(name.to_owned()))
    }
}

fn client_from_row(row: &Row<'_>) -> duckdb::Result<Client> {
    Ok(Client {
        id: ClientId::new(row.get(0)?),
        name: row.get(1)?,
        created_at: Timestamp::from_unix_micros(row.get(2)?),
    })
}

fn site_from_row(row: &Row<'_>) -> duckdb::Result<Site> {
    Ok(Site {
        id: SiteId::new(row.get(0)?),
        client_id: ClientId::new(row.get(1)?),
        client_name: row.get(2)?,
        name: row.get(3)?,
        format_spec: row.get(4)?,
        created_at: Timestamp::from_unix_micros(row.get(5)?),
        settings: row.get(6)?,
    })
}

#[cfg(test)]
mod tests;
