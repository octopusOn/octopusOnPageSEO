use std::borrow::Cow;
use std::io::{self, Read};

use memchr::{memchr, memchr_iter, memrchr};

use super::DEFAULT_MAX_LINE_BYTES;

/// Default size of a [`Chunk`]; large enough to amortise per-chunk overhead, small enough
/// to keep a few dozen chunks in flight across worker threads.
pub const DEFAULT_CHUNK_BYTES: usize = 4 * 1024 * 1024;

/// A block of whole lines read by [`ChunkReader`], suitable for parsing on another thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    first_line: u64,
    bytes: Vec<u8>,
    ends_truncated: bool,
    max_line_bytes: usize,
}

impl Chunk {
    /// 1-based number of the first line in this chunk, relative to its stream.
    pub fn first_line(&self) -> u64 {
        self.first_line
    }

    /// Raw size in bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the chunk holds no bytes.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Iterates over the lines of the chunk.
    pub fn lines(&self) -> ChunkLines<'_> {
        ChunkLines {
            chunk: self,
            position: 0,
            number: self.first_line,
        }
    }
}

/// One line of a [`Chunk`], borrowing its raw bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkLine<'a> {
    /// 1-based line number within the stream.
    pub number: u64,
    /// Raw line bytes without terminator, cut at the maximum line length.
    pub bytes: &'a [u8],
    /// The line exceeded the maximum length and `bytes` holds only its beginning.
    pub truncated: bool,
}

impl<'a> ChunkLine<'a> {
    /// The line as text; invalid UTF-8 is replaced with `U+FFFD`.
    pub fn text(&self) -> Cow<'a, str> {
        String::from_utf8_lossy(self.bytes)
    }
}

/// Iterator returned by [`Chunk::lines`].
#[derive(Debug)]
pub struct ChunkLines<'a> {
    chunk: &'a Chunk,
    position: usize,
    number: u64,
}

impl<'a> Iterator for ChunkLines<'a> {
    type Item = ChunkLine<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let bytes = &self.chunk.bytes;
        if self.position >= bytes.len() {
            return None;
        }
        let rest = &bytes[self.position..];
        let (mut line, is_last) = match memchr(b'\n', rest) {
            Some(at) => (&rest[..at], false),
            None => (rest, true),
        };
        self.position += line.len() + 1;
        if let [head @ .., b'\r'] = line {
            line = head;
        }
        let max = self.chunk.max_line_bytes;
        let truncated = line.len() > max || (is_last && self.chunk.ends_truncated);
        let number = self.number;
        self.number += 1;
        Some(ChunkLine {
            number,
            bytes: &line[..line.len().min(max)],
            truncated,
        })
    }
}

/// Splits a stream into [`Chunk`]s of whole lines.
///
/// Memory stays bounded on hostile input: a line longer than the chunk size is emitted
/// truncated and the rest of it is discarded.
#[derive(Debug)]
pub struct ChunkReader<R> {
    inner: R,
    chunk_bytes: usize,
    max_line_bytes: usize,
    carry: Vec<u8>,
    next_line: u64,
    discarding: bool,
    eof: bool,
}

impl<R: Read> ChunkReader<R> {
    /// Creates a reader with [`DEFAULT_CHUNK_BYTES`] and [`DEFAULT_MAX_LINE_BYTES`].
    pub fn new(inner: R) -> Self {
        Self::with_sizes(inner, DEFAULT_CHUNK_BYTES, DEFAULT_MAX_LINE_BYTES)
    }

    /// Creates a reader with custom chunk and line limits (minimum 1 byte each).
    pub fn with_sizes(inner: R, chunk_bytes: usize, max_line_bytes: usize) -> Self {
        Self {
            inner,
            chunk_bytes: chunk_bytes.max(1),
            max_line_bytes: max_line_bytes.max(1),
            carry: Vec::new(),
            next_line: 1,
            discarding: false,
            eof: false,
        }
    }

    /// Reads the next chunk, or `None` at end of stream.
    ///
    /// # Errors
    /// Propagates I/O and decompression errors from the underlying reader.
    pub fn next_chunk(&mut self) -> io::Result<Option<Chunk>> {
        loop {
            let mut buffer = std::mem::take(&mut self.carry);
            self.fill(&mut buffer)?;
            if self.discarding {
                self.skip_discarded_tail(&mut buffer);
                self.carry = buffer;
                continue;
            }
            if buffer.is_empty() {
                return Ok(None);
            }
            return Ok(Some(self.split_chunk(buffer)));
        }
    }

    /// Drops bytes up to and including the next newline, ending discard mode once the
    /// newline (or end of stream) is reached.
    fn skip_discarded_tail(&mut self, buffer: &mut Vec<u8>) {
        if let Some(at) = memchr(b'\n', buffer) {
            buffer.drain(..=at);
            self.discarding = false;
        } else {
            buffer.clear();
            self.discarding = !self.eof;
        }
    }

    fn split_chunk(&mut self, mut buffer: Vec<u8>) -> Chunk {
        let mut ends_truncated = false;
        if !self.eof {
            if let Some(at) = memrchr(b'\n', &buffer) {
                self.carry = buffer.split_off(at + 1);
            } else {
                ends_truncated = true;
                self.discarding = true;
            }
        }
        let first_line = self.next_line;
        let complete_lines = memchr_iter(b'\n', &buffer).count() as u64;
        let has_partial_tail = buffer.last().is_some_and(|&byte| byte != b'\n');
        self.next_line += complete_lines + u64::from(has_partial_tail);
        Chunk {
            first_line,
            bytes: buffer,
            ends_truncated,
            max_line_bytes: self.max_line_bytes,
        }
    }

    fn fill(&mut self, buffer: &mut Vec<u8>) -> io::Result<()> {
        let start = buffer.len();
        if start >= self.chunk_bytes || self.eof {
            return Ok(());
        }
        buffer.resize(self.chunk_bytes, 0);
        let mut filled = start;
        while filled < buffer.len() {
            match self.inner.read(&mut buffer[filled..]) {
                Ok(0) => {
                    self.eof = true;
                    break;
                }
                Ok(read) => filled += read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        buffer.truncate(filled);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_all(input: &[u8], chunk: usize, max_line: usize) -> Vec<(u64, String, bool)> {
        let mut reader = ChunkReader::with_sizes(input, chunk, max_line);
        let mut lines = Vec::new();
        while let Some(chunk) = reader.next_chunk().unwrap() {
            for line in chunk.lines() {
                lines.push((line.number, line.text().into_owned(), line.truncated));
            }
        }
        lines
    }

    fn expected(rows: &[(u64, &str, bool)]) -> Vec<(u64, String, bool)> {
        rows.iter()
            .map(|&(n, text, truncated)| (n, text.to_owned(), truncated))
            .collect()
    }

    #[test]
    fn matches_line_semantics_for_every_chunk_size() {
        let input = b"a\r\nbb\n\nccc\ndddd";
        let want = expected(&[
            (1, "a", false),
            (2, "bb", false),
            (3, "", false),
            (4, "ccc", false),
            (5, "dddd", false),
        ]);
        for chunk in 5..20 {
            assert_eq!(read_all(input, chunk, 100), want, "chunk size {chunk}");
        }
    }

    #[test]
    fn overlong_line_in_chunk_is_cut_at_max() {
        let lines = read_all(b"0123456789\nok\n", 64, 4);
        assert_eq!(lines, expected(&[(1, "0123", true), (2, "ok", false)]));
    }

    #[test]
    fn line_longer_than_chunk_is_truncated_and_skipped() {
        let lines = read_all(b"ab\n0123456789abcdef\ncd\n", 6, 100);
        assert_eq!(
            lines,
            expected(&[(1, "ab", false), (2, "012345", true), (3, "cd", false)])
        );
    }

    #[test]
    fn overlong_final_line_without_newline_is_flagged() {
        let lines = read_all(b"0123456789", 4, 100);
        assert_eq!(lines, expected(&[(1, "0123", true)]));
    }

    #[test]
    fn empty_input_has_no_chunks() {
        let mut reader = ChunkReader::new(&b""[..]);
        assert_eq!(reader.next_chunk().unwrap(), None);
    }
}
