//! Allocation-free parsers for individual field values.

use std::net::IpAddr;

/// Request-line components: method, target, protocol.
pub(crate) struct RequestLine<'a> {
    pub(crate) method: &'a str,
    pub(crate) target: &'a str,
    pub(crate) protocol: &'a str,
}

/// Returns `None` for the conventional "absent" markers `-` and empty strings.
pub(crate) fn non_dash(value: &str) -> Option<&str> {
    match value {
        "" | "-" => None,
        other => Some(other),
    }
}

pub(crate) fn parse_ip(value: &str) -> Option<IpAddr> {
    value.parse().ok()
}

/// Picks the first valid address of an `X-Forwarded-For` list (`client, proxy1, proxy2`).
pub(crate) fn parse_forwarded_for(value: &str) -> Option<IpAddr> {
    value.split(',').find_map(|part| parse_ip(part.trim()))
}

/// Status codes are exactly three digits; `000`–`099` are rejected.
pub(crate) fn parse_status(value: &str) -> Option<u16> {
    let bytes = value.as_bytes();
    if bytes.len() != 3 || !bytes.iter().all(u8::is_ascii_digit) || bytes[0] == b'0' {
        return None;
    }
    value.parse().ok()
}

/// Parses a plain unsigned integer (no sign, no whitespace).
pub(crate) fn parse_u64(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

/// Parses `-` as `Ok(None)` and digits as `Ok(Some(n))`.
pub(crate) fn parse_optional_u64(value: &str) -> Result<Option<u64>, ()> {
    match non_dash(value) {
        None => Ok(None),
        Some(digits) => parse_u64(digits).map(Some).ok_or(()),
    }
}

/// Parses a non-negative decimal number and scales it to an integer unit.
///
/// `scale_digits` is the number of decimal places that become integral, e.g. seconds →
/// microseconds uses `6`, milliseconds → microseconds uses `3`. Extra fractional digits are
/// truncated. Avoids floating point so results are exact and deterministic.
pub(crate) fn parse_scaled_decimal(value: &str, scale_digits: u32) -> Option<u64> {
    let (int_part, frac_part) = value.split_once('.').unwrap_or((value, ""));
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    let int_value = if int_part.is_empty() {
        0
    } else {
        parse_u64(int_part)?
    };
    if !frac_part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut frac_value: u64 = 0;
    let mut digits = frac_part.bytes();
    for _ in 0..scale_digits {
        let digit = digits.next().map_or(0, |b| u64::from(b - b'0'));
        frac_value = frac_value * 10 + digit;
    }
    int_value
        .checked_mul(10_u64.pow(scale_digits))?
        .checked_add(frac_value)
}

/// Splits `GET /path?q=1 HTTP/1.1` into its components.
///
/// Accepts HTTP/0.9-style `GET /path` (empty protocol). Rejects anything whose method is
/// not an uppercase token, which filters out TLS handshakes and binary garbage that
/// servers log as the request line.
pub(crate) fn parse_request_line(value: &str) -> Option<RequestLine<'_>> {
    let (method, rest) = value.split_once(' ')?;
    if !is_method_token(method) || rest.is_empty() {
        return None;
    }
    let (target, protocol) = match rest.rsplit_once(' ') {
        Some((target, protocol)) if protocol.starts_with("HTTP/") => (target, protocol),
        _ => (rest, ""),
    };
    if target.is_empty() {
        return None;
    }
    Some(RequestLine {
        method,
        target,
        protocol,
    })
}

/// Splits an absolute request URL (`https://host:443/p?q`, as logged by load balancers) into
/// the host and the target; relative targets are returned unchanged without a host. A URL
/// without a path yields `/`; one with only a query yields `?q` (see [`with_root`]).
pub(crate) fn split_absolute_url(value: &str) -> (Option<&str>, &str) {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .or_else(|| value.strip_prefix("HTTPS://"))
        .or_else(|| value.strip_prefix("HTTP://"))
    else {
        return (None, value);
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = if host_port.starts_with('[') {
        host_port
            .split_once(']')
            .map_or(host_port, |(h, _)| h.trim_start_matches('['))
    } else {
        host_port.split_once(':').map_or(host_port, |(h, _)| h)
    };
    let target = match &rest[end..] {
        "" => "/",
        path => path,
    };
    (non_dash(host), target)
}

/// Prefixes a query-only target (`?q`) with `/`.
pub(crate) fn with_root(target: &str) -> std::borrow::Cow<'_, str> {
    if target.starts_with('?') {
        std::borrow::Cow::Owned(format!("/{target}"))
    } else {
        std::borrow::Cow::Borrowed(target)
    }
}

/// Turns `+` into spaces (IIS writes user agents this way) without touching `%` escapes.
pub(crate) fn decode_escapes_plus_only(value: &str) -> std::borrow::Cow<'_, str> {
    if value.contains('+') {
        std::borrow::Cow::Owned(value.replace('+', " "))
    } else {
        std::borrow::Cow::Borrowed(value)
    }
}

/// Decodes `%XX` escapes (invalid escapes are kept); `plus_is_space` also turns `+` into a
/// space, as IIS writes user agents. Borrows when nothing changes.
pub(crate) fn decode_escapes(value: &str, plus_is_space: bool) -> std::borrow::Cow<'_, str> {
    let needs = value.contains('%') || (plus_is_space && value.contains('+'));
    if !needs {
        return std::borrow::Cow::Borrowed(value);
    }
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = |b: u8| char::from(b).to_digit(16);
                if let (Some(high), Some(low)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                    out.push(u8::try_from(high * 16 + low).unwrap_or(b'?'));
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            b'+' if plus_is_space => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    std::borrow::Cow::Owned(String::from_utf8_lossy(&out).into_owned())
}

fn is_method_token(method: &str) -> bool {
    (1..=16).contains(&method.len())
        && method
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_must_be_three_digits() {
        assert_eq!(parse_status("200"), Some(200));
        assert_eq!(parse_status("499"), Some(499));
        assert_eq!(parse_status("20"), None);
        assert_eq!(parse_status("2000"), None);
        assert_eq!(parse_status("000"), None);
        assert_eq!(parse_status("-"), None);
    }

    #[test]
    fn scaled_decimal_is_exact() {
        assert_eq!(parse_scaled_decimal("0.002", 6), Some(2_000));
        assert_eq!(parse_scaled_decimal("1.5", 6), Some(1_500_000));
        assert_eq!(parse_scaled_decimal("12", 3), Some(12_000));
        assert_eq!(parse_scaled_decimal("0.1234567", 6), Some(123_456));
        assert_eq!(parse_scaled_decimal(".5", 6), Some(500_000));
        assert_eq!(parse_scaled_decimal("", 6), None);
        assert_eq!(parse_scaled_decimal("-1", 6), None);
        assert_eq!(parse_scaled_decimal("1.x", 6), None);
    }

    #[test]
    fn request_line_variants() {
        let full = parse_request_line("GET /a?b=1 HTTP/2.0").unwrap();
        assert_eq!(
            (full.method, full.target, full.protocol),
            ("GET", "/a?b=1", "HTTP/2.0")
        );

        let http09 = parse_request_line("GET /legacy").unwrap();
        assert_eq!((http09.target, http09.protocol), ("/legacy", ""));

        let spaced = parse_request_line("GET /a b HTTP/1.1").unwrap();
        assert_eq!(spaced.target, "/a b");

        assert!(parse_request_line("-").is_none());
        assert!(parse_request_line("\\x16\\x03\\x01 /").is_none());
        assert!(parse_request_line("get / HTTP/1.1").is_none());
    }

    #[test]
    fn absolute_urls_split_into_host_and_target() {
        assert_eq!(
            split_absolute_url("https://www.example.com:443/a/b?x=1"),
            (Some("www.example.com"), "/a/b?x=1")
        );
        assert_eq!(
            split_absolute_url("http://example.com"),
            (Some("example.com"), "/")
        );
        assert_eq!(
            split_absolute_url("http://u:p@example.com?q"),
            (Some("example.com"), "?q")
        );
        assert_eq!(
            split_absolute_url("https://[2001:db8::1]:8443/x"),
            (Some("2001:db8::1"), "/x")
        );
        assert_eq!(split_absolute_url("/relative?x"), (None, "/relative?x"));
        assert_eq!(with_root("?q"), "/?q");
    }

    #[test]
    fn escapes_are_decoded() {
        assert_eq!(
            decode_escapes("Mozilla/5.0%20(compatible)", false),
            "Mozilla/5.0 (compatible)"
        );
        assert_eq!(
            decode_escapes("Mozilla/5.0+(compatible;+Googlebot/2.1)", true),
            "Mozilla/5.0 (compatible; Googlebot/2.1)"
        );
        assert_eq!(decode_escapes("100%", false), "100%");
        assert_eq!(decode_escapes("a%zzb", false), "a%zzb");
        assert!(matches!(
            decode_escapes("plain", true),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn forwarded_for_takes_first_valid_address() {
        assert_eq!(
            parse_forwarded_for("unknown, 203.0.113.9, 10.0.0.1"),
            "203.0.113.9".parse().ok()
        );
        assert_eq!(parse_forwarded_for("-"), None);
    }
}
