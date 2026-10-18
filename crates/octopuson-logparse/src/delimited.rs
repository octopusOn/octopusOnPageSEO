//! CSV and TSV access logs with a header row (exports from CDNs, hosting panels, BigQuery or
//! spreadsheets).
//!
//! Columns are recognised by name (see [`crate::structured`]); the delimiter is a tab, `;`
//! or `,`, picked from the header. Fields may be quoted with `"` (`""` escapes a quote);
//! quoted values spanning several lines are not supported.

use std::borrow::Cow;

use crate::error::{ParseError, ParseErrorKind};
use crate::format::LogFormat;
use crate::record::LogRecord;
use crate::structured::{Parts, Role, role_of};

/// A delimited log with a known header.
#[derive(Debug, Clone)]
pub struct DelimitedFormat {
    delimiter: char,
    header: String,
    columns: Vec<Option<(Role, u8)>>,
}

impl DelimitedFormat {
    /// Builds a format from a header row. `None` unless it names a time, client IP, URL and
    /// status column.
    pub fn from_header(header: &str) -> Option<Self> {
        let header = header
            .trim_start_matches('\u{feff}')
            .trim_end_matches(['\r', '\n']);
        let delimiter = if header.contains('\t') {
            '\t'
        } else if header.matches(';').count() > header.matches(',').count() {
            ';'
        } else {
            ','
        };
        let names = split_delimited(header, delimiter)?;
        if names.len() < 3 {
            return None;
        }
        let columns: Vec<Option<(Role, u8)>> = names.iter().map(|name| role_of(name)).collect();
        let roles: Vec<Role> = columns.iter().flatten().map(|(role, _)| *role).collect();
        Parts::has_required(&roles).then(|| Self {
            delimiter,
            header: header.to_owned(),
            columns,
        })
    }

    /// The format whose header is the first non-empty line of `lines`.
    pub fn from_sample<S: AsRef<str>>(lines: &[S]) -> Option<Self> {
        let first = lines
            .iter()
            .map(AsRef::as_ref)
            .find(|line| !line.trim().is_empty())?;
        Self::from_header(first)
    }

    /// The header row this format reads.
    pub fn header(&self) -> &str {
        &self.header
    }
}

impl LogFormat for DelimitedFormat {
    #[allow(
        clippy::unnecessary_literal_bound,
        reason = "signature is fixed by the LogFormat trait"
    )]
    fn id(&self) -> &str {
        "csv"
    }

    #[allow(
        clippy::unnecessary_literal_bound,
        reason = "signature is fixed by the LogFormat trait"
    )]
    fn display_name(&self) -> &str {
        "CSV / TSV with header"
    }

    fn specificity(&self) -> usize {
        self.columns.iter().flatten().count()
    }

    fn parse<'a>(&self, line: &'a str) -> Result<LogRecord<'a>, ParseError> {
        let line = line
            .trim_start_matches('\u{feff}')
            .trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            return Err(ParseError::new(ParseErrorKind::EmptyLine, 0));
        }
        if line == self.header {
            return Err(ParseError::new(ParseErrorKind::Comment, 0));
        }
        let values = split_delimited(line, self.delimiter)
            .ok_or(ParseError::new(ParseErrorKind::LayoutMismatch, 0))?;
        if values.len() != self.columns.len() {
            return Err(ParseError::new(ParseErrorKind::LayoutMismatch, 0));
        }
        let mut parts = Parts::default();
        for (value, column) in values.into_iter().zip(&self.columns) {
            if let Some((role, priority)) = column {
                parts.set(*role, *priority, value);
            }
        }
        parts.into_record()
    }
}

/// Splits one delimited line, unquoting `"…"` fields; `None` for an unterminated quote.
pub(crate) fn split_delimited(line: &str, delimiter: char) -> Option<Vec<Cow<'_, str>>> {
    let mut fields = Vec::new();
    let mut rest = line;
    loop {
        if let Some(quoted) = rest.strip_prefix('"') {
            let mut value = String::new();
            let mut tail = quoted;
            loop {
                let end = tail.find('"')?;
                value.push_str(&tail[..end]);
                tail = &tail[end + 1..];
                if let Some(after) = tail.strip_prefix('"') {
                    value.push('"');
                    tail = after;
                } else {
                    break;
                }
            }
            fields.push(Cow::Owned(value));
            match tail.strip_prefix(delimiter) {
                Some(next) => rest = next,
                None if tail.trim().is_empty() => return Some(fields),
                None => return None,
            }
        } else {
            let Some(end) = rest.find(delimiter) else {
                fields.push(Cow::Borrowed(rest));
                return Some(fields);
            };
            fields.push(Cow::Borrowed(&rest[..end]));
            rest = &rest[end + delimiter.len_utf8()..];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_quoted_fields() {
        let fields = split_delimited(r#"a,"b, ""c""",,d"#, ',').unwrap();
        assert_eq!(fields, ["a", r#"b, "c""#, "", "d"]);
        assert!(split_delimited(r#"a,"open"#, ',').is_none());
        assert_eq!(split_delimited("x\ty", '\t').unwrap(), ["x", "y"]);
    }

    #[test]
    fn parses_rows_by_header_names() {
        let csv = "timestamp,client_ip,method,url,status,bytes,user_agent,referer\n\
2026-10-07T18:39:53Z,66.249.66.1,GET,/a?x=1,200,535,\"Mozilla/5.0 (compatible; Googlebot/2.1)\",-";
        let lines: Vec<&str> = csv.lines().collect();
        let format = DelimitedFormat::from_sample(&lines).unwrap();
        assert_eq!(
            format.parse(lines[0]).unwrap_err().kind,
            ParseErrorKind::Comment
        );
        let record = format.parse(lines[1]).unwrap();
        assert_eq!(record.target, "/a?x=1");
        assert_eq!(record.status, 200);
        assert_eq!(record.bytes, Some(535));
        assert_eq!(
            record.user_agent.as_deref(),
            Some("Mozilla/5.0 (compatible; Googlebot/2.1)")
        );
        assert_eq!(record.referer, None);
    }

    #[test]
    fn semicolons_tabs_and_separate_date_columns() {
        let format = DelimitedFormat::from_header("Date;Time;IP;Request;Status").unwrap();
        let record = format
            .parse("2026-10-07;18:39:53;10.0.0.1;GET /x HTTP/1.1;404")
            .unwrap();
        assert_eq!(record.timestamp.to_string(), "2026-10-07T18:39:53Z");
        assert_eq!(record.target, "/x");
        let tsv = DelimitedFormat::from_header("time\tremote_addr\trequest_uri\tstatus").unwrap();
        assert_eq!(
            tsv.parse("1791398393\t10.0.0.1\t/\t200").unwrap().status,
            200
        );
        assert_eq!(
            tsv.parse("1791398393\t10.0.0.1\t/").unwrap_err().kind,
            ParseErrorKind::LayoutMismatch
        );
    }

    #[test]
    fn ignores_headers_that_are_not_logs() {
        assert!(DelimitedFormat::from_header("name,price,stock").is_none());
        assert!(
            DelimitedFormat::from_header("66.249.66.1 - - [07/Oct/2026:18:39:53 +0000]").is_none()
        );
    }
}
