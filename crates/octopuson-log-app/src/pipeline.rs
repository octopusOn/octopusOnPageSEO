//! Parallel parse pipeline shared by `inspect` and `import`.
//!
//! The calling thread reads [`Chunk`]s and runs the (sequential) sink while worker threads
//! map the previous batch of chunks, so decompression, parsing and storage overlap. Results
//! reach the sink in input order, which keeps line numbers and samples deterministic.

use std::io::{self, Read};

use octopuson_logparse::input::{Chunk, ChunkReader};
use octopuson_logparse::{LogFormat, LogRecord, ParseErrorKind};
use rayon::prelude::*;

use crate::stats::ChunkStats;

/// Streams `reader` through `map` (in parallel) and `sink` (in order).
///
/// The sink may stop processing by returning an error, e.g. on cancellation.
pub(crate) fn run<T, E, M, S>(
    reader: &mut dyn Read,
    map: M,
    mut sink: S,
    read_error: impl Fn(io::Error) -> E,
) -> Result<(), E>
where
    T: Send,
    M: Fn(&Chunk) -> T + Sync,
    S: FnMut(T) -> Result<(), E>,
{
    let mut chunks = ChunkReader::new(reader);
    let batch_size = rayon::current_num_threads().max(1);
    let mut pending = read_batch(&mut chunks, batch_size).map_err(&read_error)?;
    let mut ready: Vec<T> = Vec::new();
    while !pending.is_empty() || !ready.is_empty() {
        let mut mapped = Vec::new();
        let (sunk, next) = rayon::in_place_scope(|scope| {
            scope.spawn(|_| mapped = pending.par_iter().map(&map).collect());
            let sunk = ready.drain(..).try_for_each(&mut sink);
            let next = match sunk {
                Ok(()) => read_batch(&mut chunks, batch_size),
                Err(_) => Ok(Vec::new()),
            };
            (sunk, next)
        });
        sunk?;
        pending = next.map_err(&read_error)?;
        ready = mapped;
    }
    Ok(())
}

fn read_batch<R: Read>(chunks: &mut ChunkReader<R>, size: usize) -> io::Result<Vec<Chunk>> {
    let mut batch = Vec::with_capacity(size);
    while batch.len() < size {
        match chunks.next_chunk()? {
            Some(chunk) => batch.push(chunk),
            None => break,
        }
    }
    Ok(batch)
}

/// Parses every line of `chunk`, calling `on_record` with each record and its raw bytes;
/// `on_record` returns whether the record is kept (`false`: filtered by site settings).
pub(crate) fn parse_chunk(
    format: &dyn LogFormat,
    chunk: &Chunk,
    max_samples: usize,
    mut on_record: impl FnMut(&LogRecord<'_>, &[u8]) -> bool,
) -> ChunkStats {
    let mut stats = ChunkStats::new(max_samples);
    for line in chunk.lines() {
        let text = line.text();
        if line.truncated {
            stats.record_skipped(line.number, ParseErrorKind::LineTooLong, &text);
            continue;
        }
        match format.parse(&text) {
            Ok(record) => {
                if on_record(&record, line.bytes) {
                    stats.record_parsed(&record);
                } else {
                    stats.record_filtered();
                }
            }
            Err(error) => stats.record_skipped(line.number, error.kind, &text),
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    #[test]
    fn results_arrive_in_input_order() {
        let mut input = String::new();
        for n in 0..200_000 {
            writeln!(input, "{n}").unwrap();
        }
        let mut seen = Vec::new();
        run(
            &mut input.as_bytes(),
            |chunk: &Chunk| (chunk.first_line(), chunk.lines().count()),
            |(first, count)| {
                seen.push((first, count));
                Ok::<_, io::Error>(())
            },
            |error| error,
        )
        .unwrap();
        let total: usize = seen.iter().map(|&(_, count)| count).sum();
        assert_eq!(total, 200_000);
        assert!(seen.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn sink_error_stops_processing() {
        let input = "a\n".repeat(10_000_000);
        let mut calls = 0;
        let result = run(
            &mut input.as_bytes(),
            |_chunk: &Chunk| (),
            |()| {
                calls += 1;
                Err("stop")
            },
            |_| "io",
        );
        assert_eq!(result, Err("stop"));
        assert_eq!(calls, 1);
    }
}
