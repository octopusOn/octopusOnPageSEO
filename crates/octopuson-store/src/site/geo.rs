//! Country and autonomous system per client IP (`ip_geo`), joined by queries on raw hits.
//!
//! The lookup itself belongs to the application (`octopuson-geo`); the store keeps one row per
//! distinct IP seen in the site's hits and the data release they were looked up with
//! (`site_meta.geo_release`). A new release re-resolves every address.

use duckdb::{Connection, OptionalExt, params};

use super::{files, maintenance, relation};
use crate::error::StoreError;
use crate::paths::SitePaths;

const RELEASE_KEY: &str = "geo_release";

/// Facts about one address supplied by an [`IpGeoLookup`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IpGeo {
    /// ISO 3166-1 alpha-2 country code.
    pub country: Option<String>,
    /// Autonomous system number.
    pub asn: Option<u32>,
    /// Organization operating the autonomous system.
    pub as_org: Option<String>,
}

/// Resolves client IPs in bulk.
pub trait IpGeoLookup: Sync {
    /// Identifies the data (e.g. `country-2026-10+asn-2026-10`); a change re-resolves all IPs.
    fn release(&self) -> &str;

    /// One answer per input, in order.
    fn lookup(&self, ips: &[String]) -> Vec<IpGeo>;
}

/// Result of [`refresh`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GeoRefresh {
    /// Addresses looked up now.
    pub resolved: u64,
    /// Whether all earlier answers were discarded because the data release changed.
    pub replaced: bool,
}

/// Data release the stored answers come from.
pub(super) fn stored_release(connection: &Connection) -> Result<Option<String>, StoreError> {
    Ok(connection
        .query_row(
            "SELECT value FROM site_meta WHERE key = ?",
            params![RELEASE_KEY],
            |row| row.get(0),
        )
        .optional()?)
}

/// Resolves every client IP without an answer (all of them after a release change).
pub(super) fn refresh(
    connection: &Connection,
    paths: &SitePaths,
    lookup: &dyn IpGeoLookup,
) -> Result<GeoRefresh, StoreError> {
    let replaced = stored_release(connection)?.as_deref() != Some(lookup.release());
    let missing: Vec<String> = if files::has_published_hits(paths)? {
        let hits = relation::query_hits(paths)?;
        let known = if replaced {
            ""
        } else {
            " EXCEPT SELECT ip FROM ip_geo"
        };
        let mut statement = connection.prepare(&format!(
            "SELECT DISTINCT ip FROM ({hits}) WHERE ip IS NOT NULL{known}"
        ))?;
        statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?
    } else {
        Vec::new()
    };
    let answers = lookup.lookup(&missing);
    maintenance::in_transaction(connection, |connection| {
        if replaced {
            connection.execute("DELETE FROM ip_geo", [])?;
        }
        {
            let mut appender = connection.appender("ip_geo")?;
            for (ip, geo) in missing.iter().zip(&answers) {
                appender.append_row(params![ip, geo.country, geo.asn, geo.as_org])?;
            }
            appender.flush()?;
        }
        connection.execute(
            "INSERT OR REPLACE INTO site_meta VALUES (?, ?)",
            params![RELEASE_KEY, lookup.release()],
        )?;
        Ok(())
    })?;
    Ok(GeoRefresh {
        resolved: missing.len() as u64,
        replaced,
    })
}
