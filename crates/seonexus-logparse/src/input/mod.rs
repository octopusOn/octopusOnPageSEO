//! Opening log files (plain or compressed) and reading bounded-length lines.
//!
//! Compression is detected from magic bytes, not file extensions, so rotated files such as
//! `access.log.1` that are actually gzip still work.

mod chunks;
mod lines;

pub use chunks::{Chunk, ChunkLine, ChunkLines, ChunkReader, DEFAULT_CHUNK_BYTES};
pub use lines::{DEFAULT_MAX_LINE_BYTES, Line, LineReader};

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

const READ_BUFFER_BYTES: usize = 256 * 1024;

/// Compression of a log stream, detected from its first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Compression {
    /// Plain text.
    None,
    /// gzip (`.gz`), including multi-member files.
    Gzip,
    /// Zstandard (`.zst`).
    Zstd,
    /// bzip2 (`.bz2`), including multi-stream files.
    Bzip2,
    /// ZIP archive; every file entry is read as a separate stream.
    Zip,
}

impl Compression {
    /// Detects compression from a file header.
    pub fn sniff(header: &[u8]) -> Self {
        match header {
            [0x1f, 0x8b, ..] => Self::Gzip,
            [0x28, 0xb5, 0x2f, 0xfd, ..] => Self::Zstd,
            [b'B', b'Z', b'h', ..] => Self::Bzip2,
            [b'P', b'K', 0x03, 0x04, ..] => Self::Zip,
            _ => Self::None,
        }
    }
}

/// Describes the stream currently being read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamInfo {
    /// Display name: the file path, or `archive.zip!/entry.log` for ZIP entries.
    pub name: String,
    /// Compression of the containing file.
    pub compression: Compression,
}

/// Errors while opening or decompressing input files.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InputError {
    /// The file could not be opened or read.
    #[error("cannot read `{path}`")]
    Io {
        /// File involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
    /// The ZIP archive is corrupt or uses an unsupported feature.
    #[error("cannot read ZIP archive `{path}`")]
    Zip {
        /// Archive involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: zip::result::ZipError,
    },
}

/// Opens `path` and calls `visit` once per contained log stream.
///
/// Plain and single-stream compressed files produce one stream; ZIP archives produce one
/// per file entry. Returning `Ok(ControlFlow::Break(()))` from `visit` stops early.
///
/// # Errors
/// Returns `E::from(InputError)` when the file cannot be opened or decoded, or any error
/// returned by `visit`.
pub fn for_each_stream<E, F>(path: &Path, mut visit: F) -> Result<(), E>
where
    E: From<InputError>,
    F: FnMut(&StreamInfo, &mut dyn BufRead) -> Result<ControlFlow<()>, E>,
{
    let io_error = |source| InputError::Io {
        path: path.to_owned(),
        source,
    };
    let file = File::open(path).map_err(io_error)?;
    let mut reader = BufReader::with_capacity(READ_BUFFER_BYTES, file);
    let compression = Compression::sniff(reader.fill_buf().map_err(io_error)?);
    let info = StreamInfo {
        name: path.display().to_string(),
        compression,
    };

    let mut decoded: Box<dyn BufRead> = match compression {
        Compression::None => Box::new(reader),
        Compression::Gzip => buffered(flate2::bufread::MultiGzDecoder::new(reader)),
        Compression::Zstd => {
            buffered(zstd::stream::read::Decoder::with_buffer(reader).map_err(io_error)?)
        }
        Compression::Bzip2 => buffered(bzip2::bufread::MultiBzDecoder::new(reader)),
        Compression::Zip => return visit_zip_entries(path, reader.into_inner(), visit),
    };
    visit(&info, decoded.as_mut()).map(|_| ())
}

fn buffered<R: Read + 'static>(reader: R) -> Box<dyn BufRead> {
    Box::new(BufReader::with_capacity(READ_BUFFER_BYTES, reader))
}

fn visit_zip_entries<E, F>(path: &Path, file: File, mut visit: F) -> Result<(), E>
where
    E: From<InputError>,
    F: FnMut(&StreamInfo, &mut dyn BufRead) -> Result<ControlFlow<()>, E>,
{
    let zip_error = |source| InputError::Zip {
        path: path.to_owned(),
        source,
    };
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error)?;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(zip_error)?;
        if entry.is_dir() {
            continue;
        }
        let entry_name = entry.name().map_err(zip_error)?;
        let info = StreamInfo {
            name: format!("{}!/{entry_name}", path.display()),
            compression: Compression::Zip,
        };
        let mut reader = BufReader::with_capacity(READ_BUFFER_BYTES, entry);
        if visit(&info, &mut reader)?.is_break() {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
