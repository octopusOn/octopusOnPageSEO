//! The SQL each kind of record is listed, read and found with, built from its description in
//! [`super::kinds`]. Names come only from those descriptions; values are always parameters.

use super::kinds::{CACHE_CLASSES, CACHE_MAX, Fill, GSC_TABLES, Key, Kind, Shape};

/// A column of the record `t`, quoted.
pub(crate) fn column(name: &str) -> String {
    format!("t.\"{name}\"")
}

/// The expression of a record's key over `t` and its parent `p`.
pub(crate) fn key(kind: &Kind) -> String {
    match kind.key {
        Key::Uid => "t.uid".to_owned(),
        Key::Parent => "p.uid".to_owned(),
        Key::Fixed(key) => format!("'{key}'"),
        Key::Column(name) => column(name),
    }
}

/// `FROM` the record's table `t`, joined to its parent `p`.
pub(crate) fn from(kind: &Kind) -> String {
    match kind.parent {
        Some(parent) => format!(
            "FROM {table} AS t {join} JOIN {parent_table} AS p ON p.id = t.\"{parent_column}\"",
            table = kind.table,
            join = if kind.optional_parent { "LEFT" } else { "" },
            parent_table = parent.table,
            parent_column = parent.column,
        ),
        None => format!("FROM {} AS t", kind.table),
    }
}

/// The conditions a travelling row meets.
fn conditions(kind: &Kind) -> String {
    let mut conditions = vec![format!("{} IS NOT NULL", key(kind))];
    if let Some(filter) = kind.filter {
        conditions.push(filter.to_owned());
    }
    if matches!(kind.shape, Shape::Cache) {
        conditions.push(format!(
            "t.class IN {CACHE_CLASSES} AND length(t.body) <= {CACHE_MAX}"
        ));
    }
    conditions.join(" AND ")
}

/// The fingerprint of a row: what is compared to notice it changed. Only ever compared on
/// the same copy, so it may differ between copies.
fn hash(kind: &Kind) -> String {
    let columns: Vec<&str> = match kind.shape {
        // A kept answer changes only when it is fetched again.
        Shape::Cache => vec!["fetched_at", "expires_at", "via"],
        _ => kind.columns.to_vec(),
    };
    let mut parts: Vec<String> = columns
        .iter()
        .map(|name| format!("coalesce(CAST({} AS VARCHAR), chr(1))", column(name)))
        .collect();
    if kind.optional_parent {
        parts.push("coalesce(p.uid, chr(1))".to_owned());
    }
    format!("md5(concat_ws(chr(31), {}))", parts.join(", "))
}

/// Where a record's parent is noted: `kind/uid`.
fn parent(kind: &Kind) -> String {
    kind.parent.map_or_else(
        || "CAST(NULL AS VARCHAR)".to_owned(),
        |parent| format!("'{}/' || p.uid", parent.kind),
    )
}

/// Every travelling record of a kind as `key, hash, parent`.
pub(crate) fn listing(kind: &Kind) -> String {
    if matches!(kind.shape, Shape::Gsc) {
        let parts: Vec<String> = GSC_TABLES
            .iter()
            .map(|table| {
                let columns: Vec<String> = table
                    .columns
                    .iter()
                    .map(|name| format!("\"{name}\""))
                    .collect();
                format!(
                    "SELECT project_id, hash('{field}', {columns}) AS h FROM {table}",
                    field = table.field,
                    columns = columns.join(", "),
                    table = table.table,
                )
            })
            .collect();
        return format!(
            "SELECT p.uid AS key,
                    md5(CAST(sum(g.h) AS VARCHAR) || ':' || CAST(count(*) AS VARCHAR)) AS hash,
                    'project/' || p.uid AS parent
             FROM ({}) AS g JOIN projects AS p ON p.id = g.project_id
             WHERE p.uid IS NOT NULL
             GROUP BY p.uid",
            parts.join(" UNION ALL ")
        );
    }
    format!(
        "SELECT DISTINCT ON (key) key, hash, parent FROM (
             SELECT {key} AS key, {hash} AS hash, {parent} AS parent {from} WHERE {conditions}
         )",
        key = key(kind),
        hash = hash(kind),
        parent = parent(kind),
        from = from(kind),
        conditions = conditions(kind),
    )
}

/// One record's `hash, parent`, by key.
pub(crate) fn one(kind: &Kind) -> String {
    format!("SELECT hash, parent FROM ({}) WHERE key = ?", listing(kind))
}

/// One record's parent uid and travelling columns, by key.
pub(crate) fn read(kind: &Kind) -> String {
    let parent = if kind.parent.is_some() {
        "p.uid"
    } else {
        "CAST(NULL AS VARCHAR)"
    };
    let columns: Vec<String> = kind.columns.iter().map(|name| column(name)).collect();
    let mut select = vec![parent.to_owned()];
    select.extend(columns);
    format!(
        "SELECT {} {} WHERE {} AND {} = ?",
        select.join(", "),
        from(kind),
        conditions(kind),
        key(kind)
    )
}

/// Filling in missing uids, when the kind has them.
pub(crate) fn fill(kind: &Kind) -> Option<String> {
    match (&kind.fill, kind.parent) {
        (Fill::None, _) => None,
        (Fill::Random, _) => Some(format!(
            "UPDATE {} SET uid = CAST(uuid() AS VARCHAR) WHERE uid IS NULL",
            kind.table
        )),
        (Fill::From(expression), Some(parent)) => Some(format!(
            "UPDATE {table} AS t SET uid = md5({expression})
             FROM {parent_table} AS p
             WHERE p.id = t.\"{parent_column}\" AND t.uid IS NULL AND p.uid IS NOT NULL",
            table = kind.table,
            parent_table = parent.table,
            parent_column = parent.column,
        )),
        (Fill::From(expression), None) => Some(format!(
            "UPDATE {table} AS t SET uid = md5({expression}) WHERE t.uid IS NULL",
            table = kind.table,
        )),
    }
}

/// The condition finding a kind's row in its table (without the `t` alias), and whether it
/// takes the key (`Some(true)`), the parent's id (`Some(false)`) or nothing (`None`).
pub(crate) fn locate(kind: &Kind) -> (String, Option<bool>) {
    match kind.key {
        Key::Uid => ("uid = ?".to_owned(), Some(true)),
        Key::Parent => (
            format!(
                "\"{}\" = ?",
                kind.parent.map_or("project_id", |parent| parent.column)
            ),
            Some(false),
        ),
        Key::Fixed(_) => ("id = 1".to_owned(), None),
        Key::Column(name) => (format!("\"{name}\" = ?"), Some(true)),
    }
}
