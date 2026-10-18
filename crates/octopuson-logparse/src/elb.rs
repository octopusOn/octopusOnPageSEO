//! AWS Elastic Load Balancing access logs: Application Load Balancer and Classic Load
//! Balancer.
//!
//! ```text
//! ALB:     type time elb client:port target:port t1 t2 t3 elb_status target_status received sent "request" "user_agent" … "domain_name" …
//! Classic: time elb client:port backend:port t1 t2 t3 elb_status backend_status received sent "request" "user_agent" ssl_cipher ssl_protocol
//! ```
//!
//! The request line holds an absolute URL (`GET https://host:443/path HTTP/1.1`); its host
//! becomes the record's host. The status is the one the load balancer returned to the client
//! and the duration the sum of the three processing times (unknown when one is `-1`).

use std::borrow::Cow;

use crate::error::{Field, ParseError, ParseErrorKind};
use crate::format::LogFormat;
use crate::record::LogRecord;
use crate::structured::{absolute_to_target, parse_ip_maybe_port};
use crate::time;
use crate::tokens::{split_tokens, unwrap};
use crate::values;

/// AWS ALB / Classic ELB access logs.
#[derive(Debug, Clone, Copy, Default)]
pub struct ElbFormat;

const ALB_TYPES: [&str; 6] = ["http", "https", "h2", "grpcs", "ws", "wss"];

impl LogFormat for ElbFormat {
    #[allow(
        clippy::unnecessary_literal_bound,
        reason = "signature is fixed by the LogFormat trait"
    )]
    fn id(&self) -> &str {
        "aws-elb"
    }

    #[allow(
        clippy::unnecessary_literal_bound,
        reason = "signature is fixed by the LogFormat trait"
    )]
    fn display_name(&self) -> &str {
        "AWS Elastic Load Balancing (ALB / Classic)"
    }

    fn specificity(&self) -> usize {
        10
    }

    fn parse<'a>(&self, line: &'a str) -> Result<LogRecord<'a>, ParseError> {
        let layout = || ParseError::new(ParseErrorKind::LayoutMismatch, 0);
        let invalid = |field| ParseError::new(ParseErrorKind::InvalidValue(field), 0);
        if line.trim().is_empty() {
            return Err(ParseError::new(ParseErrorKind::EmptyLine, 0));
        }
        let tokens = split_tokens(line.trim_end()).ok_or_else(layout)?;
        let alb = tokens.first().is_some_and(|t| ALB_TYPES.contains(t));
        let base = usize::from(alb);
        let field = |offset: usize| tokens.get(base + offset).copied().ok_or_else(layout);

        let timestamp = time::parse_iso8601(field(0)?).ok_or(invalid(Field::Timestamp))?;
        let client_ip = parse_ip_maybe_port(field(2)?).ok_or(invalid(Field::ClientIp))?;
        let duration_micros = [field(4)?, field(5)?, field(6)?]
            .iter()
            .map(|t| values::parse_scaled_decimal(t, 6))
            .sum::<Option<u64>>();
        let status = values::parse_status(field(7)?).ok_or(invalid(Field::Status))?;
        let bytes = values::parse_optional_u64(field(10)?).map_err(|()| invalid(Field::Bytes))?;
        let request =
            values::parse_request_line(unwrap(field(11)?)).ok_or(invalid(Field::Request))?;
        let user_agent = values::non_dash(unwrap(field(12)?)).map(Cow::Borrowed);
        let mut host = None;
        let target = absolute_to_target(Cow::Borrowed(request.target), &mut host);
        if host.is_none() && alb {
            // `domain_name` (TLS SNI) when the request line had no host.
            host = tokens
                .get(base + 17)
                .and_then(|t| values::non_dash(unwrap(t)))
                .map(Cow::Borrowed);
        }
        Ok(LogRecord {
            timestamp,
            client_ip,
            method: Cow::Borrowed(request.method),
            target,
            protocol: Cow::Borrowed(request.protocol),
            status,
            bytes,
            referer: None,
            user_agent,
            duration_micros,
            host,
            router: None,
            upstream: None,
            request_seq: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_application_load_balancer_lines() {
        let line = r#"https 2026-10-07T18:39:53.207245Z app/my-lb/50dc6c495c0c9188 66.249.66.1:2817 10.0.0.1:80 0.000 0.001 0.000 200 200 34 366 "GET https://www.example.com:443/blog/post?x=1 HTTP/1.1" "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)" ECDHE-RSA-AES128-GCM-SHA256 TLSv1.2 arn:aws:elasticloadbalancing:us-east-2:123456789012:targetgroup/my-targets/73e2d6bc24d8a067 "Root=1-58337262-36d228ad5d99923122bbe354" "www.example.com" "arn:aws:acm:us-east-2:123456789012:certificate/12345678-1234-1234-1234-123456789012" 0 2026-10-07T18:39:53.206000Z "forward" "-" "-" "10.0.0.1:80" "200" "-" "-" TID_1234"#;
        let record = ElbFormat.parse(line).unwrap();
        assert_eq!(record.client_ip.to_string(), "66.249.66.1");
        assert_eq!(record.target, "/blog/post?x=1");
        assert_eq!(record.host.as_deref(), Some("www.example.com"));
        assert_eq!(record.status, 200);
        assert_eq!(record.bytes, Some(366));
        assert_eq!(record.duration_micros, Some(1_000));
        assert!(record.user_agent.as_deref().unwrap().contains("Googlebot"));
    }

    #[test]
    fn parses_classic_load_balancer_lines_and_failed_backends() {
        let line = r#"2026-10-07T18:39:53.207245Z my-loadbalancer 192.168.131.39:2817 - -1 -1 -1 504 0 0 0 "GET http://www.example.com:80/ HTTP/1.1" "curl/8.7" - -"#;
        let record = ElbFormat.parse(line).unwrap();
        assert_eq!(record.status, 504);
        assert_eq!(record.target, "/");
        assert_eq!(record.duration_micros, None);
        assert_eq!(record.bytes, Some(0));
    }

    #[test]
    fn rejects_other_formats() {
        let combined =
            r#"66.249.66.1 - - [07/Oct/2026:18:39:53 +0000] "GET / HTTP/1.1" 200 5 "-" "x""#;
        assert!(ElbFormat.parse(combined).is_err());
    }
}
