//! W3C extended log files: Microsoft IIS and Amazon CloudFront.
//!
//! The column layout comes from the `#Fields:` directive at the top of every file, so a
//! format instance is built from that line (see [`W3cFormat::from_sample`]). Presets with
//! the IIS default fields and the CloudFront standard fields cover files without a header.
//! Directive lines (`#Software`, `#Fields`, …) are reported as
//! [`ParseErrorKind::Comment`]. Times are UTC, as the W3C format prescribes.

use std::borrow::Cow;

use crate::error::{ParseError, ParseErrorKind};
use crate::format::LogFormat;
use crate::record::LogRecord;
use crate::structured::{Parts, Role, role_of};
use crate::values;

/// IIS default fields (`W3C` logging with the default field selection).
const IIS_FIELDS: &str = "date time s-ip cs-method cs-uri-stem cs-uri-query s-port cs-username c-ip cs(User-Agent) cs(Referer) sc-status sc-substatus sc-win32-status time-taken";

/// CloudFront standard (legacy) access-log fields.
const CLOUDFRONT_FIELDS: &str = "date time x-edge-location sc-bytes c-ip cs-method cs(Host) cs-uri-stem sc-status cs(Referer) cs(User-Agent) cs-uri-query cs(Cookie) x-edge-result-type x-edge-request-id x-host-header cs-protocol cs-bytes time-taken x-forwarded-for ssl-protocol ssl-cipher x-edge-response-result-type cs-protocol-version fle-status fle-encrypted-fields c-port time-to-first-byte x-edge-detailed-result-type sc-content-type sc-content-len sc-range-start sc-range-end";

/// A W3C extended log with a known column layout.
#[derive(Debug, Clone)]
pub struct W3cFormat {
    id: &'static str,
    name: &'static str,
    fields: String,
    columns: Vec<Option<(Role, u8)>>,
    /// CloudFront percent-encodes user agents and referers; IIS writes spaces as `+`.
    cloudfront: bool,
}

impl W3cFormat {
    /// Builds a format from a field list such as `date time c-ip cs-method …` (the value of a
    /// `#Fields:` directive). `None` if it lacks a time, client IP, URL or status column.
    pub fn from_fields(fields: &str) -> Option<Self> {
        Self::build("w3c", "W3C extended (#Fields header)", fields)
    }

    /// The format described by the first `#Fields:` directive among `lines`.
    pub fn from_sample<S: AsRef<str>>(lines: &[S]) -> Option<Self> {
        lines.iter().find_map(|line| {
            let line = line.as_ref().trim();
            let fields = line
                .strip_prefix("#Fields:")
                .or_else(|| line.strip_prefix("#fields:"))?;
            Self::from_fields(fields)
        })
    }

    /// IIS with its default fields, for files whose header was lost.
    pub fn iis() -> Option<Self> {
        Self::build("iis", "Microsoft IIS (W3C default fields)", IIS_FIELDS)
    }

    /// CloudFront standard logs.
    pub fn cloudfront() -> Option<Self> {
        Self::build(
            "cloudfront",
            "Amazon CloudFront (standard logs)",
            CLOUDFRONT_FIELDS,
        )
    }

    /// The field list this format reads.
    pub fn fields(&self) -> &str {
        &self.fields
    }

    fn build(id: &'static str, name: &'static str, fields: &str) -> Option<Self> {
        let names: Vec<&str> = fields.split_whitespace().collect();
        let columns: Vec<Option<(Role, u8)>> = names.iter().map(|name| role_of(name)).collect();
        let roles: Vec<Role> = columns.iter().flatten().map(|(role, _)| *role).collect();
        if !Parts::has_required(&roles) {
            return None;
        }
        Some(Self {
            id,
            name,
            fields: names.join(" "),
            columns,
            cloudfront: names
                .iter()
                .any(|name| name.starts_with("x-edge-") || *name == "x-host-header"),
        })
    }

    fn decode<'a>(&self, role: Role, value: &'a str) -> Cow<'a, str> {
        match (role, self.cloudfront) {
            (Role::UserAgent | Role::Referer, true) => {
                // CloudFront encodes some characters twice (`%2520`).
                match values::decode_escapes(value, false) {
                    Cow::Owned(once) if once.contains('%') => {
                        Cow::Owned(values::decode_escapes(&once, false).into_owned())
                    }
                    decoded => decoded,
                }
            }
            (Role::UserAgent, false) => values::decode_escapes_plus_only(value),
            _ => Cow::Borrowed(value),
        }
    }
}

impl LogFormat for W3cFormat {
    fn id(&self) -> &str {
        self.id
    }

    fn display_name(&self) -> &str {
        self.name
    }

    fn specificity(&self) -> usize {
        self.columns.iter().flatten().count()
    }

    fn parse<'a>(&self, line: &'a str) -> Result<LogRecord<'a>, ParseError> {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            return Err(ParseError::new(ParseErrorKind::EmptyLine, 0));
        }
        if line.starts_with('#') {
            return Err(ParseError::new(ParseErrorKind::Comment, 0));
        }
        let separator = if line.contains('\t') { '\t' } else { ' ' };
        let mut parts = Parts::default();
        let mut count = 0;
        for (index, value) in line.split(separator).enumerate() {
            count += 1;
            if let Some(Some((role, priority))) = self.columns.get(index) {
                parts.set(*role, *priority, self.decode(*role, value));
            }
        }
        if count != self.columns.len() {
            return Err(ParseError::new(ParseErrorKind::LayoutMismatch, 0));
        }
        parts.into_record()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IIS: &str = "#Software: Microsoft Internet Information Services 10.0
#Version: 1.0
#Date: 2026-10-07 18:39:53
#Fields: date time s-ip cs-method cs-uri-stem cs-uri-query s-port cs-username c-ip cs(User-Agent) cs(Referer) sc-status sc-substatus sc-win32-status time-taken
2026-10-07 18:39:53 10.0.0.5 GET /products/item-1 color=red 443 - 66.249.66.1 Mozilla/5.0+(compatible;+Googlebot/2.1;++http://www.google.com/bot.html) - 200 0 0 15";

    #[test]
    fn reads_the_layout_from_the_fields_directive() {
        let lines: Vec<&str> = IIS.lines().collect();
        let format = W3cFormat::from_sample(&lines).unwrap();
        assert_eq!(format.id(), "w3c");
        assert_eq!(
            format.parse(lines[0]).unwrap_err().kind,
            ParseErrorKind::Comment
        );
        let record = format.parse(lines[4]).unwrap();
        assert_eq!(record.timestamp.to_string(), "2026-10-07T18:39:53Z");
        assert_eq!(record.client_ip.to_string(), "66.249.66.1");
        assert_eq!(record.target, "/products/item-1?color=red");
        assert_eq!(record.status, 200);
        assert_eq!(record.duration_micros, Some(15_000));
        assert_eq!(
            record.user_agent.as_deref(),
            Some("Mozilla/5.0 (compatible; Googlebot/2.1;  http://www.google.com/bot.html)")
        );
        assert_eq!(record.referer, None);
        // The IIS preset reads the same line without the header.
        assert_eq!(W3cFormat::iis().unwrap().parse(lines[4]).unwrap(), record);
    }

    #[test]
    fn cloudfront_lines_are_tab_separated_and_encoded() {
        let format = W3cFormat::cloudfront().unwrap();
        let line = "2026-10-07\t18:39:53\tIST50-C1\t5120\t66.249.66.1\tGET\td111111abcdef8.cloudfront.net\t/index.html\t200\thttps://www.google.com/\tMozilla/5.0%20(compatible;%20Googlebot/2.1)\t-\t-\tHit\tSOX4xwn4XV6Q4rgb7XiVGOHms_BGlTAC4KyHmureZmBNrjGdRLiNIQ==\twww.example.com\thttps\t23\t0.001\t-\tTLSv1.3\tTLS_AES_128_GCM_SHA256\tHit\tHTTP/2.0\t-\t-\t11040\t0.001\tHit\ttext/html\t5120\t-\t-";
        let record = format.parse(line).unwrap();
        assert_eq!(record.host.as_deref(), Some("www.example.com"));
        assert_eq!(
            record.user_agent.as_deref(),
            Some("Mozilla/5.0 (compatible; Googlebot/2.1)")
        );
        assert_eq!(record.referer.as_deref(), Some("https://www.google.com/"));
        assert_eq!(record.bytes, Some(5120));
        assert_eq!(record.duration_micros, Some(1_000));
        assert_eq!(record.protocol, "HTTP/2.0");
        assert_eq!(record.target, "/index.html");
    }

    #[test]
    fn rejects_headers_without_the_essentials_and_wrong_column_counts() {
        assert!(W3cFormat::from_fields("date time s-ip").is_none());
        let format = W3cFormat::iis().unwrap();
        assert_eq!(
            format.parse("2026-10-07 18:39:53 GET /").unwrap_err().kind,
            ParseErrorKind::LayoutMismatch
        );
    }
}
