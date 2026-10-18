//! Importing a single file inside one store transaction.

use std::ops::ControlFlow;
use std::path::Path;

use std::collections::HashMap;

use octopuson_botverify::Classifier;
use octopuson_logparse::LogFormat;
use octopuson_logparse::input::{Chunk, for_each_stream};
use octopuson_store::{
    BotTag, FileFingerprint, HitBatch, HitBatchBuilder, ImportOptions, ImportRecord, ImportedFile,
    SiteStore, SkippedSample, StoreError,
};
use xxhash_rust::xxh3::xxh3_64;

use super::{FileOutcome, ImportError, ImportProgress};
use crate::bots::{BotContext, tag};
use crate::cancel::CancelToken;
use crate::pipeline::{self, parse_chunk};
use crate::settings::{Decision, ImportFilter};
use crate::stats::{ChunkStats, ParseStats};

/// Everything a single-file import needs besides the file itself.
pub(super) struct FileImportContext<'a> {
    pub(super) format: &'a dyn LogFormat,
    pub(super) format_spec: &'a str,
    pub(super) max_error_samples: usize,
    pub(super) options: ImportOptions,
    pub(super) bots: &'a BotContext,
    pub(super) filter: &'a ImportFilter,
    pub(super) progress: &'a dyn ImportProgress,
    pub(super) cancel: &'a CancelToken,
}

/// Output of one parallel map step.
struct ParsedChunk {
    stats: ChunkStats,
    batch: Result<HitBatch, StoreError>,
    bytes: u64,
}

pub(super) fn import_file(
    store: &mut SiteStore,
    path: &Path,
    fingerprint: FileFingerprint,
    context: &FileImportContext<'_>,
) -> Result<FileOutcome, ImportError> {
    let (format, max_samples) = (context.format, context.max_error_samples);
    let classifier = context.bots.classifier.0;
    let mut session = store.begin_import(context.options, &context.bots.verifier)?;
    let mut stats = ParseStats::new(max_samples);
    for_each_stream::<ImportError, _>(path, |info, reader| {
        stats.begin_stream(&info.name);
        pipeline::run(
            reader,
            |chunk| parse_to_batch(format, &classifier, context.filter, max_samples, chunk),
            |parsed: ParsedChunk| {
                if context.cancel.is_cancelled() {
                    return Err(ImportError::Cancelled);
                }
                context.progress.advanced(parsed.stats.lines, parsed.bytes);
                stats.merge(parsed.stats);
                session.append(parsed.batch?)?;
                Ok(())
            },
            |source| ImportError::Read {
                stream: info.name.clone(),
                source,
            },
        )?;
        Ok(ControlFlow::Continue(()))
    })?;

    if stats.parsed_lines() == 0 && stats.total_lines() > 0 {
        // Nothing matched the site's format: roll back instead of remembering the file, so
        // it can be imported again after the format is corrected.
        drop(session);
        return Ok(FileOutcome::Unrecognized {
            summary: stats.into_summary(),
        });
    }
    let file = ImportedFile {
        fingerprint,
        path: path.display().to_string(),
    };
    let skipped = stats.skipped_by_reason();
    let samples = skipped_samples(&stats);
    let outcome = session.commit(&ImportRecord {
        format_spec: context.format_spec,
        file: &file,
        total_lines: stats.total_lines(),
        parsed_lines: stats.parsed_lines(),
        first_request: stats.first(),
        last_request: stats.last(),
        skipped: &skipped,
        samples: &samples,
    })?;
    Ok(FileOutcome::Imported {
        import_id: outcome.import_id,
        inserted_rows: outcome.inserted_rows,
        duplicate_rows: outcome.duplicate_rows,
        summary: stats.into_summary(),
    })
}

/// Runs on worker threads, so it only receives `Sync` inputs. User agents repeat heavily,
/// so their classification is cached per chunk.
fn parse_to_batch(
    format: &dyn LogFormat,
    classifier: &Classifier,
    filter: &ImportFilter,
    max_samples: usize,
    chunk: &Chunk,
) -> ParsedChunk {
    let mut builder = HitBatchBuilder::new();
    let mut bots: HashMap<Box<str>, Option<BotTag>> = HashMap::new();
    let stats = parse_chunk(format, chunk, max_samples, |record, raw_line| {
        let bot = record.user_agent.as_deref().and_then(|ua| {
            if let Some(known) = bots.get(ua) {
                return *known;
            }
            let found = classifier.classify(ua).map(tag);
            bots.insert(ua.into(), found);
            found
        });
        match filter.decide(record, bot) {
            Decision::Keep => builder.push(record, xxh3_64(raw_line), bot),
            Decision::Rewrite(rewritten) => builder.push(&rewritten, xxh3_64(raw_line), bot),
            Decision::Drop => return false,
        }
        true
    });
    ParsedChunk {
        stats,
        batch: builder.finish(),
        bytes: chunk.len() as u64,
    }
}

fn skipped_samples(stats: &ParseStats) -> Vec<SkippedSample> {
    stats
        .samples()
        .iter()
        .map(|sample| SkippedSample {
            stream: sample.stream.clone(),
            line_number: sample.line_number,
            reason: sample.reason.clone(),
            excerpt: sample.excerpt.clone(),
        })
        .collect()
}
