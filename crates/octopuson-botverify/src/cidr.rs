//! CIDR prefixes and fast membership tests over many of them.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

/// An IPv4 or IPv6 network such as `66.249.64.0/19`. A bare address is a single-host network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IpNet {
    address: IpAddr,
    prefix_len: u8,
}

/// A prefix could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid IP prefix `{0}`")]
pub struct IpNetError(String);

impl IpNet {
    /// Creates a network; host bits of `address` are cleared.
    ///
    /// # Errors
    /// When `prefix_len` exceeds the address width.
    pub fn new(address: IpAddr, prefix_len: u8) -> Result<Self, IpNetError> {
        let width = width(address);
        if prefix_len > width {
            return Err(IpNetError(format!("{address}/{prefix_len}")));
        }
        let (start, _) = bounds(address, prefix_len);
        Ok(Self {
            address: from_bits(address, start),
            prefix_len,
        })
    }

    /// Network address.
    pub fn address(&self) -> IpAddr {
        self.address
    }

    /// Prefix length in bits.
    pub fn prefix_len(&self) -> u8 {
        self.prefix_len
    }

    /// Whether `ip` lies in this network (IPv4-mapped IPv6 addresses match IPv4 networks).
    pub fn contains(&self, ip: IpAddr) -> bool {
        let ip = canonical(ip);
        if ip.is_ipv4() != self.address.is_ipv4() {
            return false;
        }
        let (start, end) = bounds(self.address, self.prefix_len);
        (start..=end).contains(&bits(ip))
    }
}

impl FromStr for IpNet {
    type Err = IpNetError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = || IpNetError(text.to_owned());
        let text = text.trim();
        let (address, prefix_len) = match text.split_once('/') {
            Some((address, len)) => (address, Some(len)),
            None => (text, None),
        };
        let address = canonical(address.parse::<IpAddr>().map_err(|_| error())?);
        let prefix_len = match prefix_len {
            Some(len) => len.parse::<u8>().map_err(|_| error())?,
            None => width(address),
        };
        Self::new(address, prefix_len).map_err(|_| error())
    }
}

impl fmt::Display for IpNet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.address, self.prefix_len)
    }
}

/// Many networks merged into sorted, non-overlapping intervals for `O(log n)` lookups.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IpRangeSet {
    v4: Vec<(u128, u128)>,
    v6: Vec<(u128, u128)>,
}

impl IpRangeSet {
    /// Builds a set from networks.
    pub fn new(networks: impl IntoIterator<Item = IpNet>) -> Self {
        let (mut v4, mut v6) = (Vec::new(), Vec::new());
        for network in networks {
            let interval = bounds(network.address, network.prefix_len);
            if network.address.is_ipv4() {
                v4.push(interval);
            } else {
                v6.push(interval);
            }
        }
        Self {
            v4: merge(v4),
            v6: merge(v6),
        }
    }

    /// Whether no network was added.
    pub fn is_empty(&self) -> bool {
        self.v4.is_empty() && self.v6.is_empty()
    }

    /// Whether `ip` lies in any network.
    pub fn contains(&self, ip: IpAddr) -> bool {
        let ip = canonical(ip);
        let intervals = if ip.is_ipv4() { &self.v4 } else { &self.v6 };
        let value = bits(ip);
        let index = intervals.partition_point(|&(start, _)| start <= value);
        index > 0 && intervals[index - 1].1 >= value
    }
}

fn merge(mut intervals: Vec<(u128, u128)>) -> Vec<(u128, u128)> {
    intervals.sort_unstable();
    let mut merged: Vec<(u128, u128)> = Vec::with_capacity(intervals.len());
    for (start, end) in intervals {
        match merged.last_mut() {
            Some(last) if start <= last.1.saturating_add(1) => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// Maps IPv4-mapped IPv6 addresses (`::ffff:1.2.3.4`) to IPv4.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        IpAddr::V4(_) => ip,
    }
}

const fn width(ip: IpAddr) -> u8 {
    match ip {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    }
}

fn bits(ip: IpAddr) -> u128 {
    match ip {
        IpAddr::V4(v4) => u128::from(u32::from(v4)),
        IpAddr::V6(v6) => u128::from(v6),
    }
}

fn from_bits(template: IpAddr, value: u128) -> IpAddr {
    match template {
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::from(u32::try_from(value).unwrap_or(u32::MAX))),
        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::from(value)),
    }
}

/// First and last address of the network as integers.
fn bounds(address: IpAddr, prefix_len: u8) -> (u128, u128) {
    let width = u32::from(width(address));
    let host_bits = width - u32::from(prefix_len);
    let host_mask = if host_bits == 0 {
        0
    } else {
        u128::MAX >> (128 - host_bits)
    };
    let value = bits(address);
    (value & !host_mask, value | host_mask)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn parses_and_normalizes_prefixes() {
        let net: IpNet = "66.249.66.17/27".parse().unwrap();
        assert_eq!(net.to_string(), "66.249.66.0/27");
        assert_eq!("1.2.3.4".parse::<IpNet>().unwrap().prefix_len(), 32);
        assert_eq!("2001:db8::/32".parse::<IpNet>().unwrap().prefix_len(), 32);
        assert_eq!(
            "::ffff:1.2.3.4/32".parse::<IpNet>().unwrap().to_string(),
            "1.2.3.4/32"
        );
        for bad in ["", "1.2.3/24", "1.2.3.4/33", "::/129", "x/1", "1.2.3.4/-1"] {
            assert!(bad.parse::<IpNet>().is_err(), "{bad}");
        }
    }

    #[test]
    fn membership_respects_boundaries_and_families() {
        let net: IpNet = "66.249.64.0/19".parse().unwrap();
        assert!(net.contains(ip("66.249.64.0")));
        assert!(net.contains(ip("66.249.95.255")));
        assert!(!net.contains(ip("66.249.96.0")));
        assert!(net.contains(ip("::ffff:66.249.66.1")));
        assert!(!net.contains(ip("2001:4860:4801:10::1")));
        let all: IpNet = "0.0.0.0/0".parse().unwrap();
        assert!(all.contains(ip("255.255.255.255")));
    }

    #[test]
    fn range_set_merges_and_finds() {
        let set = IpRangeSet::new(
            [
                "10.0.0.0/25",
                "10.0.0.128/25",
                "10.0.2.0/24",
                "2001:db8::/48",
            ]
            .iter()
            .map(|n| n.parse().unwrap()),
        );
        assert_eq!(set.v4.len(), 2, "adjacent halves merge");
        assert!(set.contains(ip("10.0.0.200")));
        assert!(!set.contains(ip("10.0.1.1")));
        assert!(set.contains(ip("10.0.2.255")));
        assert!(set.contains(ip("2001:db8:0:ffff::1")));
        assert!(!set.contains(ip("2001:db9::1")));
        assert!(IpRangeSet::default().is_empty());
        assert!(!IpRangeSet::default().contains(ip("1.1.1.1")));
    }

    proptest! {
        #[test]
        fn set_agrees_with_networks(
            nets in prop::collection::vec((any::<u32>(), 8_u8..=32), 1..20),
            probe in any::<u32>(),
        ) {
            let nets: Vec<IpNet> = nets
                .into_iter()
                .map(|(addr, len)| IpNet::new(IpAddr::V4(Ipv4Addr::from(addr)), len).unwrap())
                .collect();
            let set = IpRangeSet::new(nets.iter().copied());
            let probe = IpAddr::V4(Ipv4Addr::from(probe));
            prop_assert_eq!(set.contains(probe), nets.iter().any(|n| n.contains(probe)));
            for net in &nets {
                prop_assert!(set.contains(net.address()));
            }
        }
    }
}
