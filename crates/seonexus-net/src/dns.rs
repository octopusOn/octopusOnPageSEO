use std::net::{IpAddr, Ipv4Addr};

use dns_lookup::{LookupError, LookupErrorKind, lookup_addr, lookup_host};
use seonexus_botverify::{DnsError, DnsResolver};

/// DNS through the operating system resolver (`getnameinfo` / `getaddrinfo`).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemDns;

impl SystemDns {
    /// Whether DNS works at all. Resolvers report "no such name" for every query while
    /// offline, which would make genuine bots look spoofed, so callers check this first.
    pub fn is_available(&self) -> bool {
        // Google Public DNS has a stable PTR record (dns.google).
        self.reverse(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)))
            .is_ok_and(|names| !names.is_empty())
    }
}

impl DnsResolver for SystemDns {
    fn reverse(&self, ip: IpAddr) -> Result<Vec<String>, DnsError> {
        let name = lookup_addr(&ip).map_err(classify)?;
        // getnameinfo echoes the address when no name exists on some platforms.
        if name.parse::<IpAddr>().is_ok() {
            return Err(DnsError::NotFound);
        }
        Ok(vec![name])
    }

    fn forward(&self, host: &str) -> Result<Vec<IpAddr>, DnsError> {
        Ok(lookup_host(host).map_err(classify)?.collect())
    }
}

fn classify(error: LookupError) -> DnsError {
    match error.kind() {
        LookupErrorKind::NoName | LookupErrorKind::NoData => DnsError::NotFound,
        _ => DnsError::Failed(std::io::Error::from(error).to_string()),
    }
}
