use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// Bytes hashed from each end of a file.
const SAMPLE_BYTES: u64 = 64 * 1024;

/// Identifies a log file's content cheaply: BLAKE3 over its size, first and last 64 KiB.
///
/// Log files only change by appending or rotating, both of which change the size or the
/// tail, so this detects re-imports without reading multi-gigabyte files twice.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileFingerprint {
    hash: [u8; 32],
    size: u64,
}

impl FileFingerprint {
    /// Fingerprints the file at `path`.
    ///
    /// # Errors
    /// Propagates I/O errors.
    pub fn of_file(path: &Path) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let size = file.metadata()?.len();
        let mut hasher = blake3::Hasher::new();
        hasher.update(&size.to_le_bytes());
        hash_range(&mut file, 0, SAMPLE_BYTES.min(size), &mut hasher)?;
        if size > SAMPLE_BYTES {
            let tail_start = size.saturating_sub(SAMPLE_BYTES).max(SAMPLE_BYTES);
            hash_range(&mut file, tail_start, size - tail_start, &mut hasher)?;
        }
        Ok(Self {
            hash: *hasher.finalize().as_bytes(),
            size,
        })
    }

    /// File size in bytes at fingerprinting time.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Lowercase hex of the hash, as stored in the database.
    pub fn to_hex(&self) -> String {
        blake3::Hash::from_bytes(self.hash).to_hex().to_string()
    }
}

impl fmt::Debug for FileFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FileFingerprint({}, {} bytes)", self.to_hex(), self.size)
    }
}

fn hash_range(
    file: &mut File,
    start: u64,
    length: u64,
    hasher: &mut blake3::Hasher,
) -> io::Result<()> {
    file.seek(SeekFrom::Start(start))?;
    let copied = io::copy(&mut file.take(length), hasher)?;
    if copied == length {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "file shrank while fingerprinting",
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn fingerprint(content: &[u8]) -> FileFingerprint {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content).unwrap();
        FileFingerprint::of_file(file.path()).unwrap()
    }

    #[test]
    fn equal_content_equal_fingerprint() {
        assert_eq!(fingerprint(b"abc"), fingerprint(b"abc"));
        assert_eq!(fingerprint(b"abc").size(), 3);
    }

    #[test]
    fn appending_changes_fingerprint() {
        let big = vec![b'x'; 200_000];
        let mut appended = big.clone();
        appended.extend_from_slice(b"\nnew line");
        assert_ne!(fingerprint(&big), fingerprint(&appended));
        assert_ne!(fingerprint(b""), fingerprint(b"a"));
    }

    #[test]
    fn tail_change_is_detected() {
        let mut a = vec![b'x'; 200_000];
        let b = a.clone();
        if let Some(last) = a.last_mut() {
            *last = b'y';
        }
        assert_ne!(fingerprint(&a), fingerprint(&b));
        assert_eq!(fingerprint(&a).to_hex().len(), 64);
    }
}
