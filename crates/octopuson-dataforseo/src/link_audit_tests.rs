//! Linking sites judged by the toxic-link rules.

use crate::backlinks::BacklinkRow;
use crate::link_audit::*;

fn link(domain: &str, spam: u32, ip: &str) -> BacklinkRow {
    BacklinkRow {
        url_from: format!("https://{domain}/page"),
        domain_from: Some(domain.to_owned()),
        url_to: Some("https://ornek.com/".to_owned()),
        anchor: Some("spor ayakkabı".to_owned()),
        dofollow: true,
        spam_score: Some(spam),
        ip: Some(ip.to_owned()),
        ..BacklinkRow::default()
    }
}

#[test]
fn networks_are_blocks_of_addresses() {
    assert_eq!(network_of("185.12.7.200").as_deref(), Some("185.12.7"));
    assert_eq!(
        network_of("2a01:4f8:c0c:1234::1").as_deref(),
        Some("2a01:4f8:c0c:1234")
    );
    assert_eq!(network_of("not an address"), None);
    assert_eq!(network_of("300.1.1.1"), None);
}

#[test]
fn rules_flag_spam_networks_tlds_farms_and_spam_anchors() {
    let mut rows = vec![
        link("haber.com.tr", 2, "10.0.0.1"),
        link("a-net.com", 10, "185.12.7.1"),
        link("b-net.com", 12, "185.12.7.2"),
        link("c-net.com", 14, "185.12.7.3"),
        link("cheap-links.xyz", 70, "91.1.1.1"),
        link("forum.org", 35, "8.8.8.8"),
        link("farm.net", 5, "1.2.3.4"),
        link("bahis-site.com", 20, "5.6.7.8"),
        link("quiet.xyz", 70, "9.9.9.9"),
        link("haber.com.tr", 80, "10.0.0.1"),
    ];
    rows[6].page_external_links = Some(400);
    rows[6].links_from_domain = Some(120);
    rows[7].anchor = Some("Canlı BAHİS siteleri".to_owned());
    rows[8].dofollow = false;
    let risks = assess(&rows);
    let find = |domain: &str| risks.iter().find(|risk| risk.domain == domain).unwrap();
    // A site is judged once, from its first link.
    assert_eq!(
        risks
            .iter()
            .filter(|risk| risk.domain == "haber.com.tr")
            .count(),
        1
    );
    assert_eq!(find("haber.com.tr").risk, Risk::Low);
    assert_eq!(find("a-net.com").flags, ["network"]);
    assert_eq!(find("a-net.com").network.as_deref(), Some("185.12.7"));
    assert_eq!(find("cheap-links.xyz").flags, ["spam", "tld"]);
    assert_eq!(find("cheap-links.xyz").risk, Risk::High);
    assert_eq!(find("forum.org").flags, ["spam_some"]);
    assert_eq!(find("forum.org").risk, Risk::Medium);
    assert_eq!(find("farm.net").flags, ["sitewide", "link_farm"]);
    assert_eq!(find("bahis-site.com").flags, ["spam_anchor"]);
    // Nofollow links weigh half.
    assert_eq!(find("quiet.xyz").score, find("cheap-links.xyz").score / 2);
    // The riskiest first.
    assert_eq!(risks[0].domain, "cheap-links.xyz");
}
