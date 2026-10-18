//! One observed near-duplicate witness per eligible page, without materializing pairs.

use std::collections::HashMap;

use duckdb::{Connection, params};
use seonexus_html::hamming_distance;

use crate::CrawlStoreError;

const MAX_DISTANCE: u32 = 3;
const MIN_WORDS: u32 = 200;

#[derive(Debug)]
struct Group {
    pages: Vec<u32>,
    text_hash: u64,
    simhash: u64,
    words: u32,
}

pub(super) fn build(connection: &Connection) -> Result<(), CrawlStoreError> {
    connection.execute("DELETE FROM content_matches", [])?;
    let groups = load(connection)?;
    let matches = witnesses(&groups);
    let mut appender = connection.appender("content_matches")?;
    for (index, witness) in matches.into_iter().enumerate() {
        if let Some((other, distance)) = witness {
            for &page in &groups[index].pages {
                appender.append_row(params![page, groups[other].pages[0], distance])?;
            }
        }
    }
    appender.flush()?;
    Ok(())
}

fn load(connection: &Connection) -> Result<Vec<Group>, CrawlStoreError> {
    let mut statement = connection.prepare("SELECT u.id,u.text_hash,u.simhash,u.word_count FROM audit_indexable u LEFT JOIN rendering r ON r.page=u.id WHERE u.word_count>=? AND u.text_hash<>0 AND u.simhash IS NOT NULL AND NOT u.truncated AND (r.page IS NULL OR (r.rendered AND NOT r.truncated)) ORDER BY u.id")?;
    let mut rows = statement.query(params![MIN_WORDS])?;
    let mut groups: Vec<Group> = Vec::new();
    let mut exact = HashMap::new();
    while let Some(row) = rows.next()? {
        let (page, text_hash, simhash, words): (u32, u64, u64, u32) =
            (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?);
        let index = *exact.entry((text_hash, simhash, words)).or_insert_with(|| {
            groups.push(Group {
                pages: Vec::new(),
                text_hash,
                simhash,
                words,
            });
            groups.len() - 1
        });
        groups[index].pages.push(page);
    }
    Ok(groups)
}

fn band(hash: u64, part: u8) -> u16 {
    u16::try_from((hash >> (u32::from(part) * 16)) & 0xffff).unwrap_or_default()
}

fn witnesses(groups: &[Group]) -> Vec<Option<(usize, u32)>> {
    // At most three changed bits leave at least one of four 16-bit bands identical.
    // Exact text groups share one index entry; repeated URLs do not create a cross product.
    let mut index: HashMap<(u8, u16), Vec<usize>> = HashMap::new();
    for (i, group) in groups.iter().enumerate() {
        for part in 0..4 {
            index
                .entry((part, band(group.simhash, part)))
                .or_default()
                .push(i);
        }
    }
    let mut result = vec![None; groups.len()];
    let mut seen = vec![usize::MAX; groups.len()];
    for (i, group) in groups.iter().enumerate() {
        if result[i].is_some() {
            continue;
        }
        'search: for part in 0..4 {
            for &j in &index[&(part, band(group.simhash, part))] {
                if i == j || seen[j] == i {
                    continue;
                }
                seen[j] = i;
                let other = &groups[j];
                if group.text_hash == other.text_hash
                    || u64::from(group.words.min(other.words)) * 5
                        < u64::from(group.words.max(other.words)) * 4
                {
                    continue;
                }
                let distance = hamming_distance(group.simhash, other.simhash);
                if distance <= MAX_DISTANCE {
                    result[i] = Some((j, distance));
                    if result[j].is_none() {
                        result[j] = Some((i, distance));
                    }
                    break 'search;
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests;
