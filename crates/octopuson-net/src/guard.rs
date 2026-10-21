//! Keeps crawls on the public internet: a server crawling addresses its users type must never
//! reach itself, its private network or a cloud metadata service.
//!
//! The guard is a name resolver: every address a host name resolves to is checked, and only
//! public ones are handed to the connection. Because the connection uses exactly the checked
//! addresses, a name that resolves differently a moment later (DNS rebinding) cannot slip
//! through, and redirects are checked like every other request.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::NextTimeout;

/// Why a request was refused, as the crawl records it.
pub const BLOCKED: &str =
    "the address is private, local or reserved; only public sites are crawled";

/// Whether an IPv4 address is on the public internet.
fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        // 0.0.0.0/8 "this network"
        || a == 0
        // 100.64.0.0/10 shared address space (carrier-grade NAT)
        || (a == 100 && (64..128).contains(&b))
        // 192.0.0.0/24 protocol assignments
        || (a == 192 && b == 0 && c == 0)
        // 198.18.0.0/15 benchmarking
        || (a == 198 && (b == 18 || b == 19))
        // 240.0.0.0/4 reserved
        || a >= 240)
}

/// Whether an IPv6 address is on the public internet; addresses carrying an IPv4 address are
/// judged by it.
fn public_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return public_v4(v4);
    }
    let segments = ip.segments();
    // 64:ff9b::/96 translates to IPv4.
    if segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
        let [a, b] = segments[6].to_be_bytes();
        let [c, d] = segments[7].to_be_bytes();
        return public_v4(Ipv4Addr::new(a, b, c, d));
    }
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        // fc00::/7 unique local (cloud metadata services live here too)
        || (segments[0] & 0xfe00) == 0xfc00
        // fe80::/10 link-local
        || (segments[0] & 0xffc0) == 0xfe80
        // ::/96 IPv4-compatible (deprecated) and 100::/64 discard
        || segments[..6] == [0, 0, 0, 0, 0, 0]
        || segments[..4] == [0x100, 0, 0, 0]
        // 2001:db8::/32 documentation
        || (segments[0] == 0x2001 && segments[1] == 0xdb8))
}

/// Whether an address is on the public internet: not loopback, private, link-local (cloud
/// metadata at 169.254.169.254), shared, documentation, multicast or otherwise reserved.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => public_v4(v4),
        IpAddr::V6(v6) => public_v6(v6),
    }
}

/// A resolver that keeps only public addresses and refuses a host that has none.
#[derive(Debug, Default)]
pub(crate) struct PublicOnly {
    inner: DefaultResolver,
}

impl Resolver for PublicOnly {
    fn resolve(
        &self,
        uri: &Uri,
        config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let resolved = self.inner.resolve(uri, config, timeout)?;
        let mut kept = self.inner.empty();
        for address in resolved.iter().filter(|address| is_public(address.ip())) {
            kept.push(*address);
        }
        if kept.is_empty() {
            return Err(ureq::Error::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                BLOCKED,
            )));
        }
        Ok(kept)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn public(text: &str) -> bool {
        is_public(text.parse().unwrap())
    }

    #[test]
    fn only_public_addresses_pass() {
        for blocked in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "0.1.2.3",
            "192.0.0.8",
            "192.0.2.1",
            "198.18.0.1",
            "224.0.0.1",
            "240.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "fc00::1",
            "fd00:ec2::254",
            "fe80::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "64:ff9b::a00:1",
            "2001:db8::1",
            "100::1",
        ] {
            assert!(!public(blocked), "{blocked} must be refused");
        }
        for open in [
            "93.184.216.34",
            "8.8.8.8",
            "172.32.0.1",
            "100.128.0.1",
            "2606:4700:4700::1111",
            "::ffff:8.8.8.8",
            "64:ff9b::808:808",
        ] {
            assert!(public(open), "{open} must pass");
        }
    }
}
