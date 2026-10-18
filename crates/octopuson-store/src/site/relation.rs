//! The `hits` relation over published Parquet files (see `octopuson_query::sql`).

use std::path::Path;

use super::files;
use crate::error::StoreError;
use crate::paths::SitePaths;
use crate::sql::path_literal;

/// Published Parquet hit files with the `month` partition key kept as text; `filename` adds
/// the source file path as a column.
fn parquet_hits(glob: &Path, filename: bool) -> Result<String, StoreError> {
    Ok(format!(
        "SELECT * FROM read_parquet({}, hive_partitioning = true, \
         hive_types = {{'month': VARCHAR}}, filename = {filename})",
        path_literal(glob)?
    ))
}

/// Typed, empty stand-in used before the first import publishes any file.
pub(super) const EMPTY_HITS: &str = "SELECT CAST(NULL AS TIMESTAMP) AS ts, \
    CAST(NULL AS VARCHAR) AS ip, CAST(NULL AS VARCHAR) AS method, CAST(NULL AS BIGINT) AS url_id, \
    CAST(NULL AS VARCHAR) AS protocol, CAST(NULL AS USMALLINT) AS status, \
    CAST(NULL AS UBIGINT) AS bytes, CAST(NULL AS VARCHAR) AS referer, \
    CAST(NULL AS BIGINT) AS ua_id, CAST(NULL AS BIGINT) AS duration_us, \
    CAST(NULL AS VARCHAR) AS host, CAST(NULL AS VARCHAR) AS router, \
    CAST(NULL AS VARCHAR) AS upstream, CAST(NULL AS UBIGINT) AS request_seq, \
    CAST(NULL AS UBIGINT) AS line_hash, CAST(NULL AS VARCHAR) AS month WHERE FALSE";

/// The `hits` relation for queries; empty before the first import.
pub(super) fn query_hits(paths: &SitePaths) -> Result<String, StoreError> {
    if files::has_published_hits(paths)? {
        parquet_hits(&files::published_glob(paths), false)
    } else {
        Ok(EMPTY_HITS.to_owned())
    }
}

/// Hits with their source file name, or `None` before the first import.
pub(super) fn hits_with_filename(paths: &SitePaths) -> Result<Option<String>, StoreError> {
    if files::has_published_hits(paths)? {
        parquet_hits(&files::published_glob(paths), true).map(Some)
    } else {
        Ok(None)
    }
}
