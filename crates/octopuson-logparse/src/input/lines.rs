use std::borrow::Cow;
use std::io::{self, BufRead};

use memchr::memchr;

/// Default upper bound for a single line. Longer lines are truncated and flagged, which
/// protects imports from unbounded memory use on corrupt or hostile input.
pub const DEFAULT_MAX_LINE_BYTES: usize = 64 * 1024;

/// One line read by [`LineReader`].
#[derive(Debug)]
pub struct Line<'a> {
    /// 1-based line number within the stream.
    pub number: u64,
    /// Line text without terminator; invalid UTF-8 is replaced with `U+FFFD`.
    pub text: Cow<'a, str>,
    /// The line exceeded the maximum length and `text` holds only its beginning.
    pub truncated: bool,
}

/// Reads lines with a reusable buffer and a hard per-line size limit.
///
/// This is a lending reader: each [`Line`] borrows the internal buffer until the next call.
#[derive(Debug)]
pub struct LineReader<R> {
    inner: R,
    buffer: Vec<u8>,
    max_line_bytes: usize,
    line_number: u64,
}

impl<R: BufRead> LineReader<R> {
    /// Creates a reader with [`DEFAULT_MAX_LINE_BYTES`].
    pub fn new(inner: R) -> Self {
        Self::with_max_line_bytes(inner, DEFAULT_MAX_LINE_BYTES)
    }

    /// Creates a reader with a custom line limit (minimum 1 byte).
    pub fn with_max_line_bytes(inner: R, max_line_bytes: usize) -> Self {
        Self {
            inner,
            buffer: Vec::with_capacity(1024),
            max_line_bytes: max_line_bytes.max(1),
            line_number: 0,
        }
    }

    /// Reads the next line, or `None` at end of stream.
    ///
    /// # Errors
    /// Propagates I/O and decompression errors from the underlying reader.
    pub fn next_line(&mut self) -> io::Result<Option<Line<'_>>> {
        self.buffer.clear();
        let mut truncated = false;
        let mut saw_any = false;
        loop {
            let available = match self.inner.fill_buf() {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            };
            if available.is_empty() {
                break;
            }
            saw_any = true;
            let newline = memchr(b'\n', available);
            let chunk_end = newline.unwrap_or(available.len());
            let room = self.max_line_bytes.saturating_sub(self.buffer.len());
            if chunk_end > room {
                truncated = true;
            }
            self.buffer
                .extend_from_slice(&available[..chunk_end.min(room)]);
            let consumed = newline.map_or(available.len(), |at| at + 1);
            self.inner.consume(consumed);
            if newline.is_some() {
                break;
            }
        }
        if !saw_any {
            return Ok(None);
        }
        if self.buffer.last() == Some(&b'\r') {
            self.buffer.pop();
        }
        self.line_number += 1;
        Ok(Some(Line {
            number: self.line_number,
            text: String::from_utf8_lossy(&self.buffer),
            truncated,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(input: &[u8], max: usize) -> Vec<(u64, String, bool)> {
        let mut reader = LineReader::with_max_line_bytes(input, max);
        let mut lines = Vec::new();
        while let Some(line) = reader.next_line().unwrap() {
            lines.push((line.number, line.text.into_owned(), line.truncated));
        }
        lines
    }

    #[test]
    fn splits_lf_and_crlf_and_last_line_without_newline() {
        let lines = collect(b"a\r\nb\n\nc", 100);
        assert_eq!(
            lines,
            vec![
                (1, "a".into(), false),
                (2, "b".into(), false),
                (3, String::new(), false),
                (4, "c".into(), false),
            ]
        );
    }

    #[test]
    fn truncates_long_lines_and_resynchronises() {
        let lines = collect(b"0123456789\nok\n", 4);
        assert_eq!(
            lines,
            vec![(1, "0123".into(), true), (2, "ok".into(), false)]
        );
    }

    #[test]
    fn invalid_utf8_is_replaced() {
        let lines = collect(b"GET /\xff\n", 100);
        assert_eq!(lines[0].1, "GET /\u{fffd}");
    }

    #[test]
    fn empty_input_has_no_lines() {
        assert_eq!(collect(b"", 100), Vec::new());
    }
}
