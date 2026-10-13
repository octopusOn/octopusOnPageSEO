use std::ops::ControlFlow;
use std::path::Path;

use seonexus_logparse::input::{InputError, LineReader, for_each_stream};

/// Reads up to `limit` non-empty lines from the first stream of `path`.
pub(crate) fn read_sample(path: &Path, limit: usize) -> Result<Vec<String>, InputError> {
    let mut sample = Vec::with_capacity(limit);
    for_each_stream::<InputError, _>(path, |_, reader| {
        let mut lines = LineReader::new(reader);
        while sample.len() < limit {
            let line = lines.next_line().map_err(|source| InputError::Io {
                path: path.to_owned(),
                source,
            })?;
            match line {
                Some(line) if !line.truncated && !line.text.trim().is_empty() => {
                    sample.push(line.text.into_owned());
                }
                Some(_) => {}
                None => break,
            }
        }
        Ok(ControlFlow::Break(()))
    })?;
    Ok(sample)
}
