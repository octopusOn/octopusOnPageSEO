//! IP address → country and autonomous system (ASN) lookups.
//!
//! Data comes from the free DB-IP Lite databases (<https://db-ip.com>, Creative Commons
//! Attribution 4.0: applications showing the results must credit "IP Geolocation by DB-IP"),
//! downloaded by the application. This crate only parses their CSV form and answers lookups;
//! it performs no I/O.
//!
//! - Country CSV rows: `ip_start,ip_end,CC` (ISO 3166-1 alpha-2; `ZZ` for unknown).
//! - ASN CSV rows: `ip_start,ip_end,asn,organization` (the organization may be quoted).
//!
//! Ranges are kept sorted per address family and searched with a binary search.

use std::collections::HashMap;
use std::net::IpAddr;

/// Credit line the DB-IP licence requires wherever results are shown.
pub const ATTRIBUTION: &str = "IP Geolocation by DB-IP (https://db-ip.com)";

/// A malformed data file.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {reason}")]
pub struct GeoError {
    /// 1-based line number.
    pub line: usize,
    /// What is wrong.
    pub reason: &'static str,
}

/// Network facts about one address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeoInfo<'a> {
    /// ISO 3166-1 alpha-2 country code.
    pub country: Option<&'a str>,
    /// Autonomous system number.
    pub asn: Option<u32>,
    /// Organization operating the autonomous system.
    pub as_org: Option<&'a str>,
}

/// Sorted, non-overlapping address ranges with a value index.
#[derive(Debug, Default)]
struct Ranges<T> {
    starts: Vec<T>,
    ends: Vec<T>,
    values: Vec<u32>,
}

impl<T: Copy + Ord> Ranges<T> {
    fn push(&mut self, start: T, end: T, value: u32) {
        self.starts.push(start);
        self.ends.push(end);
        self.values.push(value);
    }

    fn sort(&mut self) {
        if self.starts.windows(2).all(|pair| pair[0] <= pair[1]) {
            return;
        }
        let mut order: Vec<usize> = (0..self.starts.len()).collect();
        order.sort_by_key(|&i| self.starts[i]);
        self.starts = order.iter().map(|&i| self.starts[i]).collect();
        self.ends = order.iter().map(|&i| self.ends[i]).collect();
        self.values = order.iter().map(|&i| self.values[i]).collect();
    }

    fn find(&self, address: T) -> Option<u32> {
        let index = self
            .starts
            .partition_point(|&start| start <= address)
            .checked_sub(1)?;
        (address <= self.ends[index]).then(|| self.values[index])
    }

    fn len(&self) -> usize {
        self.starts.len()
    }
}

/// One lookup table (country or ASN) for both address families.
#[derive(Debug, Default)]
struct Table {
    v4: Ranges<u32>,
    v6: Ranges<u128>,
}

impl Table {
    fn push(&mut self, start: IpAddr, end: IpAddr, value: u32) -> Result<(), &'static str> {
        match (start, end) {
            (IpAddr::V4(start), IpAddr::V4(end)) if start <= end => {
                self.v4.push(start.into(), end.into(), value);
            }
            (IpAddr::V6(start), IpAddr::V6(end)) if start <= end => {
                self.v6.push(start.into(), end.into(), value);
            }
            _ => return Err("range ends before it starts or mixes address families"),
        }
        Ok(())
    }

    fn find(&self, ip: IpAddr) -> Option<u32> {
        match ip {
            IpAddr::V4(v4) => self.v4.find(v4.into()),
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => self.v4.find(v4.into()),
                None => self.v6.find(v6.into()),
            },
        }
    }

    fn sort(&mut self) {
        self.v4.sort();
        self.v6.sort();
    }

    fn len(&self) -> usize {
        self.v4.len() + self.v6.len()
    }
}

/// Country and ASN lookups.
#[derive(Debug, Default)]
pub struct GeoDb {
    countries: Table,
    /// Two-letter codes indexed by the country table's values.
    codes: Vec<[u8; 2]>,
    asns: Table,
    /// `(asn, organization index)` indexed by the ASN table's values.
    systems: Vec<(u32, u32)>,
    organizations: Vec<Box<str>>,
}

impl GeoDb {
    /// An empty database (lookups find nothing).
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the rows of a DB-IP country CSV.
    ///
    /// # Errors
    /// [`GeoError`] for a malformed line.
    pub fn load_countries(&mut self, csv: &str) -> Result<(), GeoError> {
        let mut index: HashMap<[u8; 2], u32> = HashMap::new();
        for (number, line) in csv.lines().enumerate() {
            let fail = |reason| GeoError {
                line: number + 1,
                reason,
            };
            let fields = split_csv(line);
            let [start, end, code, ..] = fields.as_slice() else {
                if line.trim().is_empty() {
                    continue;
                }
                return Err(fail("expected ip_start,ip_end,country"));
            };
            let (start, end) = (
                parse_ip(start).ok_or(fail("invalid start address"))?,
                parse_ip(end).ok_or(fail("invalid end address"))?,
            );
            let code = code.trim().as_bytes();
            let [a, b] = code else {
                return Err(fail("country code is not two letters"));
            };
            let code = [a.to_ascii_uppercase(), b.to_ascii_uppercase()];
            if code == *b"ZZ" {
                continue;
            }
            let value = *index.entry(code).or_insert_with(|| {
                self.codes.push(code);
                u32::try_from(self.codes.len() - 1).unwrap_or(u32::MAX)
            });
            self.countries.push(start, end, value).map_err(fail)?;
        }
        self.countries.sort();
        Ok(())
    }

    /// Adds the rows of a DB-IP ASN CSV.
    ///
    /// # Errors
    /// [`GeoError`] for a malformed line.
    pub fn load_asns(&mut self, csv: &str) -> Result<(), GeoError> {
        let mut organizations: HashMap<String, u32> = HashMap::new();
        let mut systems: HashMap<(u32, u32), u32> = HashMap::new();
        for (number, line) in csv.lines().enumerate() {
            let fail = |reason| GeoError {
                line: number + 1,
                reason,
            };
            let fields = split_csv(line);
            let [start, end, asn, rest @ ..] = fields.as_slice() else {
                if line.trim().is_empty() {
                    continue;
                }
                return Err(fail("expected ip_start,ip_end,asn,organization"));
            };
            let (start, end) = (
                parse_ip(start).ok_or(fail("invalid start address"))?,
                parse_ip(end).ok_or(fail("invalid end address"))?,
            );
            let asn: u32 = asn.trim().parse().map_err(|_| fail("invalid AS number"))?;
            let name = rest.join(",");
            let name = name.trim();
            let organization = if let Some(&index) = organizations.get(name) {
                index
            } else {
                self.organizations.push(name.into());
                let index = u32::try_from(self.organizations.len() - 1).unwrap_or(u32::MAX);
                organizations.insert(name.to_owned(), index);
                index
            };
            let value = *systems.entry((asn, organization)).or_insert_with(|| {
                self.systems.push((asn, organization));
                u32::try_from(self.systems.len() - 1).unwrap_or(u32::MAX)
            });
            self.asns.push(start, end, value).map_err(fail)?;
        }
        self.asns.sort();
        Ok(())
    }

    /// Country and AS of `ip`.
    pub fn lookup(&self, ip: IpAddr) -> GeoInfo<'_> {
        let country = self
            .countries
            .find(ip)
            .and_then(|i| self.codes.get(usize::try_from(i).ok()?))
            .and_then(|code| std::str::from_utf8(code).ok());
        let system = self
            .asns
            .find(ip)
            .and_then(|i| self.systems.get(usize::try_from(i).ok()?));
        GeoInfo {
            country,
            asn: system.map(|(asn, _)| *asn),
            as_org: system
                .and_then(|(_, org)| self.organizations.get(usize::try_from(*org).ok()?))
                .map(AsRef::as_ref)
                .filter(|name: &&str| !name.is_empty()),
        }
    }

    /// Number of country and ASN ranges.
    pub fn len(&self) -> (usize, usize) {
        (self.countries.len(), self.asns.len())
    }

    /// Whether no data is loaded.
    pub fn is_empty(&self) -> bool {
        self.len() == (0, 0)
    }
}

fn parse_ip(text: &str) -> Option<IpAddr> {
    text.trim().parse().ok()
}

/// Splits one RFC 4180 line (quoted fields may contain commas; `""` is a quote).
fn split_csv(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.trim_end_matches(['\r', '\n']).chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', true) => quoted = false,
            ('"', false) if field.is_empty() => quoted = true,
            (',', false) => fields.push(std::mem::take(&mut field)),
            (c, _) => field.push(c),
        }
    }
    fields.push(field);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    const COUNTRIES: &str = "1.0.0.0,1.0.0.255,AU\n\
8.8.8.0,8.8.8.255,US\n\
78.160.0.0,78.191.255.255,TR\n\
2001:4860::,2001:4860:ffff:ffff:ffff:ffff:ffff:ffff,US\n\
10.0.0.0,10.255.255.255,ZZ\n";

    const ASNS: &str = "1.0.0.0,1.0.0.255,13335,\"Cloudflare, Inc.\"\n\
66.249.64.0,66.249.95.255,15169,Google LLC\n\
2001:4860::,2001:4860:ffff:ffff:ffff:ffff:ffff:ffff,15169,Google LLC\n";

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn looks_up_countries_and_systems() {
        let mut db = GeoDb::new();
        db.load_countries(COUNTRIES).unwrap();
        db.load_asns(ASNS).unwrap();
        assert_eq!(db.len(), (4, 3));
        assert_eq!(db.lookup(ip("78.170.1.2")).country, Some("TR"));
        assert_eq!(db.lookup(ip("8.8.8.8")).country, Some("US"));
        let cloudflare = db.lookup(ip("1.0.0.1"));
        assert_eq!(
            (cloudflare.country, cloudflare.asn, cloudflare.as_org),
            (Some("AU"), Some(13_335), Some("Cloudflare, Inc."))
        );
        let google = db.lookup(ip("2001:4860:4801::1"));
        assert_eq!((google.country, google.asn), (Some("US"), Some(15_169)));
        assert_eq!(db.lookup(ip("66.249.66.1")).as_org, Some("Google LLC"));
        // IPv4-mapped IPv6 addresses use the IPv4 table.
        assert_eq!(db.lookup(ip("::ffff:8.8.8.8")).country, Some("US"));
        // Unknown (`ZZ`) and absent ranges.
        assert_eq!(
            db.lookup(ip("10.1.1.1")),
            GeoInfo {
                country: None,
                asn: None,
                as_org: None
            }
        );
        assert_eq!(db.lookup(ip("9.9.9.9")).country, None);
    }

    #[test]
    fn unsorted_input_is_sorted() {
        let mut db = GeoDb::new();
        db.load_countries("8.8.8.0,8.8.8.255,US\n1.0.0.0,1.0.0.255,AU\n")
            .unwrap();
        assert_eq!(db.lookup(ip("1.0.0.7")).country, Some("AU"));
        assert_eq!(db.lookup(ip("8.8.8.7")).country, Some("US"));
    }

    #[test]
    fn rejects_malformed_lines() {
        let mut db = GeoDb::new();
        assert_eq!(db.load_countries("1.0.0.0,AU\n").unwrap_err().line, 1);
        assert_eq!(
            db.load_countries("1.0.0.0,1.0.0.9,AUS\n")
                .unwrap_err()
                .reason,
            "country code is not two letters"
        );
        assert_eq!(
            db.load_countries("1.0.0.9,1.0.0.0,AU\n")
                .unwrap_err()
                .reason,
            "range ends before it starts or mixes address families"
        );
        assert_eq!(
            db.load_asns("1.0.0.0,1.0.0.9,x,Org\n").unwrap_err().reason,
            "invalid AS number"
        );
        assert!(db.is_empty());
    }

    #[test]
    fn splits_quoted_csv() {
        assert_eq!(
            split_csv(r#"a,"b, c","d ""e""""#),
            ["a", "b, c", r#"d "e""#]
        );
    }
}
