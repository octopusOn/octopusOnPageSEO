use super::*;
use crate::error::{Field, ParseErrorKind};

const TRAEFIK_LINE: &str = r#"57.141.20.59 - - [07/Oct/2026:18:39:53 +0000] "GET /_next/image?url=%2Fflags%2Fgb.svg&w=828&q=75 HTTP/2.0" 200 535 "-" "Mozilla/5.0 (compatible; meta-externalagent/1.1 (+https://developers.facebook.com/docs/sharing/webmasters/crawler))" 12771189 "https-0-u10hd94shx9vwxprkmfyq0yw@docker" "http://172.18.0.6:3000" 5ms"#;

fn nginx(format: &str) -> TemplateFormat {
    TemplateFormat::from_nginx("custom", "Custom", format).unwrap()
}

fn apache(format: &str) -> TemplateFormat {
    TemplateFormat::from_apache("custom", "Custom", format).unwrap()
}

#[test]
fn traefik_preset_extracts_proxy_fields() {
    let record = presets::traefik_clf().unwrap().parse(TRAEFIK_LINE).unwrap();
    assert_eq!(record.request_seq, Some(12_771_189));
    assert_eq!(
        record.router.as_deref(),
        Some("https-0-u10hd94shx9vwxprkmfyq0yw@docker")
    );
    assert_eq!(record.upstream.as_deref(), Some("http://172.18.0.6:3000"));
    assert_eq!(record.duration_micros, Some(5_000));
    assert!(record.referer.is_none());
}

#[test]
fn combined_rejects_traefik_line_as_trailing_data() {
    let error = presets::combined()
        .unwrap()
        .parse(TRAEFIK_LINE)
        .unwrap_err();
    assert_eq!(error.kind, ParseErrorKind::TrailingData);
}

#[test]
fn common_rejects_combined_line_as_trailing_data() {
    let line = r#"1.2.3.4 - - [07/Oct/2026:18:39:53 +0000] "GET / HTTP/1.1" 200 15 "-" "UA""#;
    let error = presets::common().unwrap().parse(line).unwrap_err();
    assert_eq!(error.kind, ParseErrorKind::TrailingData);
}

#[test]
fn apache_escaped_quotes_stay_inside_value() {
    let line = r#"1.2.3.4 - - [07/Oct/2026:18:39:53 +0000] "GET / HTTP/1.1" 200 1 "-" "Bot \"quoted\" agent""#;
    let record = presets::combined().unwrap().parse(line).unwrap();
    assert_eq!(
        record.user_agent.as_deref(),
        Some(r#"Bot \"quoted\" agent"#)
    );
}

#[test]
fn nginx_custom_with_forwarded_for_and_request_time() {
    let format = nginx(
        r#"log_format seo '$remote_addr - $remote_user [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent" "$http_x_forwarded_for" $request_time';"#,
    );
    let line = r#"10.0.0.2 - - [07/Oct/2026:18:39:53 +0000] "GET /a HTTP/1.1" 404 0 "-" "Googlebot" "66.249.66.1, 10.0.0.2" 0.125"#;
    let record = format.parse(line).unwrap();
    assert_eq!(record.client_ip.to_string(), "66.249.66.1");
    assert_eq!(record.duration_micros, Some(125_000));
    assert_eq!(record.status, 404);
}

#[test]
fn nginx_path_and_args_are_joined() {
    let format = nginx("$remote_addr [$time_iso8601] $request_method $uri?$args $status");
    let record = format
        .parse("1.2.3.4 [2026-10-07T18:39:53+00:00] GET /p?a=1 200")
        .unwrap();
    assert_eq!(record.method, "GET");
    assert_eq!(record.target, "/p?a=1");
}

#[test]
fn apache_vhost_combined_with_microseconds() {
    let format = apache(r#"%v %h %l %u %t "%r" %>s %O "%{Referer}i" "%{User-Agent}i" %D"#);
    let line = r#"example.com 1.2.3.4 - - [07/Oct/2026:18:39:53 +0000] "HEAD / HTTP/1.1" 301 - "-" "curl/8" 1834"#;
    let record = format.parse(line).unwrap();
    assert_eq!(record.host.as_deref(), Some("example.com"));
    assert_eq!(record.bytes, None);
    assert_eq!(record.duration_micros, Some(1_834));
}

#[test]
fn reports_invalid_field_with_offset() {
    let line = r#"1.2.3.4 - - [07/Oct/2026:18:39:53 +0000] "GET / HTTP/1.1" 2x0 1"#;
    let error = presets::common().unwrap().parse(line).unwrap_err();
    assert_eq!(error.kind, ParseErrorKind::InvalidValue(Field::Status));
    assert_eq!(&line[error.offset..error.offset + 3], "2x0");
}

#[test]
fn malformed_request_line_is_rejected() {
    let line = r#"1.2.3.4 - - [07/Oct/2026:18:39:53 +0000] "\x16\x03\x01\x02" 400 157"#;
    let error = presets::common().unwrap().parse(line).unwrap_err();
    assert_eq!(error.kind, ParseErrorKind::InvalidValue(Field::Request));
}

#[test]
fn empty_and_whitespace_lines() {
    let format = presets::common().unwrap();
    assert_eq!(
        format.parse("").unwrap_err().kind,
        ParseErrorKind::EmptyLine
    );
    assert_eq!(
        format.parse("  \t").unwrap_err().kind,
        ParseErrorKind::EmptyLine
    );
}

#[test]
fn compile_errors() {
    let adjacent = TemplateFormat::from_nginx("x", "x", "$remote_addr$status");
    assert!(matches!(adjacent, Err(TemplateError::AdjacentFields(..))));

    let missing = TemplateFormat::from_nginx("x", "x", "$remote_addr [$time_local] $status");
    assert_eq!(
        missing.unwrap_err(),
        TemplateError::MissingRequired(Field::Request)
    );

    let empty = TemplateFormat::from_apache("x", "x", "just text");
    assert_eq!(empty.unwrap_err(), TemplateError::Empty);
}

#[test]
fn specificity_orders_presets() {
    let traefik = presets::traefik_clf().unwrap().specificity();
    let combined = presets::combined().unwrap().specificity();
    let common = presets::common().unwrap().specificity();
    assert!(traefik > combined && combined > common);
}
