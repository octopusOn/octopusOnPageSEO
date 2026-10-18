//! Helpers for SQL that cannot use bound parameters (DuckDB `COPY` targets and file lists).

use std::path::Path;

use crate::error::StoreError;

/// Quotes `value` as a SQL string literal.
pub(crate) fn string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Quotes a filesystem path as a SQL string literal.
pub(crate) fn path_literal(path: &Path) -> Result<String, StoreError> {
    path.to_str()
        .map(string_literal)
        .ok_or_else(|| StoreError::NonUtf8Path(path.to_owned()))
}

/// Builds a SQL list literal (`['a', 'b']`) of paths.
pub(crate) fn path_list_literal<'a>(
    paths: impl IntoIterator<Item = &'a Path>,
) -> Result<String, StoreError> {
    let items = paths
        .into_iter()
        .map(path_literal)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("[{}]", items.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_quotes() {
        assert_eq!(string_literal("it's"), "'it''s'");
        assert_eq!(
            path_list_literal([Path::new("/a/b.parquet"), Path::new("/c'd")]).unwrap(),
            "['/a/b.parquet', '/c''d']"
        );
    }
}
