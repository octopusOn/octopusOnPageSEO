//! WHOIS (RFC 3912) for top-level domains without RDAP, such as `.tr`.
//!
//! The registry's server is found by asking `whois.iana.org` about the TLD (`refer:` line);
//! its answer for the domain is free text, from which the creation date, expiry date and
//! registrar are read by their usual labels.

use crate::dates::{Day, parse_date};
use crate::{IntelError, top_level};

/// IANA's WHOIS server, which refers to each TLD's registry server.
pub const IANA_SERVER: &str = "whois.iana.org";

/// Something that asks a WHOIS server (TCP port 43).
pub trait WhoisPort: Send + Sync {
    /// Sends `query` to `server` and returns the answer.
    ///
    /// # Errors
    /// A message when the server cannot be reached.
    fn query(&self, server: &str, query: &str) -> Result<String, String>;
}

/// What a WHOIS answer says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WhoisRecord {
    /// Creation date.
    pub created: Option<Day>,
    /// Expiry date.
    pub expires: Option<Day>,
    /// Registrar.
    pub registrar: Option<String>,
}

/// The registry server IANA refers to for a TLD.
pub fn parse_referral(answer: &str) -> Option<String> {
    answer.lines().find_map(|line| {
        let (label, value) = line.split_once(':')?;
        let label = label.trim().to_ascii_lowercase();
        (label == "refer" || label == "whois")
            .then(|| value.trim().to_owned())
            .filter(|server| !server.is_empty() && !server.contains(char::is_whitespace))
    })
}

/// Labels of the creation date, lowercase, without trailing dots.
const CREATED: &[&str] = &[
    "creation date",
    "created on",
    "created",
    "registered on",
    "registration date",
    "domain registration date",
    "registered",
    "domain create date",
    "record created",
    "kayıt tarihi",
    "oluşturulma tarihi",
];

const EXPIRES: &[&str] = &[
    "registry expiry date",
    "expiration date",
    "expires on",
    "expiry date",
    "expires",
    "paid-till",
    "domain expiration date",
    "bitiş tarihi",
];

const REGISTRAR: &[&str] = &["registrar", "sponsoring registrar", "registrar name"];

/// Reads a WHOIS answer: `Label: value`, `Label......: value` or `[Label] value` lines.
pub fn parse_record(answer: &str) -> WhoisRecord {
    let mut record = WhoisRecord::default();
    for line in answer.lines() {
        let Some((label, value)) = split_line(line) else {
            continue;
        };
        if record.created.is_none() && CREATED.contains(&label.as_str()) {
            record.created = parse_date(value);
        } else if record.expires.is_none() && EXPIRES.contains(&label.as_str()) {
            record.expires = parse_date(value);
        } else if record.registrar.is_none() && REGISTRAR.contains(&label.as_str()) {
            let value = value.trim();
            if !value.is_empty() {
                record.registrar = Some(value.to_owned());
            }
        }
    }
    record
}

fn split_line(line: &str) -> Option<(String, &str)> {
    let line = line.trim().trim_start_matches(['*', '%', '#', ' ']);
    if let Some(rest) = line.strip_prefix('[') {
        let (label, value) = rest.split_once(']')?;
        return Some((label.trim().to_lowercase(), value));
    }
    let (label, value) = line.split_once(':')?;
    let label = label.trim().trim_end_matches('.').trim().to_lowercase();
    Some((label, value.trim_start_matches(['.', ' '])))
}

/// Looks a domain up: IANA's referral for its TLD, then the registry server.
///
/// # Errors
/// [`IntelError::Unsupported`] when IANA names no server; [`IntelError::Network`] when a
/// server cannot be reached; [`IntelError::NotFound`] when the answer has no creation date.
pub fn lookup(port: &dyn WhoisPort, domain: &str) -> Result<WhoisRecord, IntelError> {
    let tld = top_level(domain);
    let referral = port.query(IANA_SERVER, tld).map_err(IntelError::Network)?;
    let server = parse_referral(&referral)
        .ok_or_else(|| IntelError::Unsupported(format!("no WHOIS server for .{tld}")))?;
    let answer = port.query(&server, domain).map_err(IntelError::Network)?;
    let record = parse_record(&answer);
    if record.created.is_none() {
        return Err(IntelError::NotFound(domain.to_owned()));
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// A `.tr` answer in the layout of the Turkish registry.
    const TR: &str = "** Domain Name: ornek.com.tr\n\n** Registrar:\nNIC Handle\t\t: abc-metu\nOrganization Name\t: Örnek Bilişim\n\n** Additional Info:\nCreated on..............: 2004-Feb-12.\nExpires on..............: 2027-Feb-11.\n";

    const GTLD: &str = "Domain Name: EXAMPLE.NET\nRegistrar: Example Registrar, Inc.\nCreation Date: 1999-03-22T05:00:00Z\nRegistry Expiry Date: 2027-03-21T04:00:00Z\n";

    struct Canned(Mutex<Vec<(String, String)>>);

    impl WhoisPort for Canned {
        fn query(&self, server: &str, query: &str) -> Result<String, String> {
            self.0
                .lock()
                .unwrap()
                .push((server.to_owned(), query.to_owned()));
            match server {
                IANA_SERVER if query == "tr" => {
                    Ok("domain: TR\nrefer: whois.trabis.gov.tr\n".to_owned())
                }
                IANA_SERVER => Ok("domain: XX\n".to_owned()),
                "whois.trabis.gov.tr" => Ok(TR.to_owned()),
                _ => Err("unreachable".to_owned()),
            }
        }
    }

    #[test]
    fn reads_records_of_different_registries() {
        let tr = parse_record(TR);
        assert_eq!(
            tr.created.map(|day| day.to_string()).as_deref(),
            Some("2004-02-12")
        );
        assert_eq!(
            tr.expires.map(|day| day.to_string()).as_deref(),
            Some("2027-02-11")
        );
        let gtld = parse_record(GTLD);
        assert_eq!(
            gtld.created.map(|day| day.to_string()).as_deref(),
            Some("1999-03-22")
        );
        assert_eq!(gtld.registrar.as_deref(), Some("Example Registrar, Inc."));
        let bracketed = parse_record("[Created on] 2010/05/01\n[Expires on] 2030/05/31\n");
        assert_eq!(
            bracketed.created.map(|day| day.to_string()).as_deref(),
            Some("2010-05-01")
        );
    }

    #[test]
    fn follows_the_iana_referral() {
        let port = Canned(Mutex::new(Vec::new()));
        let record = lookup(&port, "ornek.com.tr").unwrap();
        assert!(record.created.is_some());
        let queries = port.0.lock().unwrap().clone();
        assert_eq!(queries[0], (IANA_SERVER.to_owned(), "tr".to_owned()));
        assert_eq!(
            queries[1],
            ("whois.trabis.gov.tr".to_owned(), "ornek.com.tr".to_owned())
        );
        assert!(matches!(
            lookup(&port, "x.xx"),
            Err(IntelError::Unsupported(_))
        ));
        assert_eq!(
            parse_referral("whois:        whois.nic.io\n").as_deref(),
            Some("whois.nic.io")
        );
    }
}
