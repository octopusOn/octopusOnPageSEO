//! Facts about the domains that rank: age, link authority, popularity.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::time::Duration;

use seonexus_core::Timestamp;
use seonexus_core::http::{HttpRequest, HttpTransport};
use seonexus_domain_intel::authority::{backlink_summary, open_pagerank};
use seonexus_domain_intel::rdap::{BOOTSTRAP_URL, Bootstrap};
use seonexus_domain_intel::whois::WhoisPort;
use seonexus_domain_intel::{AgeSource, Day, IntelError, rdap, tranco, wayback, whois};
use seonexus_serp_store::DomainFact;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::files::write_atomically;
use crate::secrets::SecretState;
use crate::{SerpAppError, SerpWorkspace};

pub use seonexus_domain_intel::tranco::NOTICE as TRANCO_NOTICE;

/// Data sources with secrets that are not SERP providers: id, secret, origin.
pub const DATA_SOURCES: &[(&str, &str, &str)] = &[(
    "open-pagerank",
    "api_key",
    "https://openpagerank.keywordseverywhere.com",
)];

const DAY_SECONDS: i64 = 86_400;
const AGE_DAYS: i64 = 180;
const AUTHORITY_DAYS: i64 = 30;
const FAILURE_DAYS: i64 = 3;
const BOOTSTRAP_DAYS: i64 = 30;

/// What is known about a domain.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DomainFacts {
    /// Registrable domain.
    pub domain: String,
    /// Registration date (or first archive capture), `YYYY-MM-DD`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// Where the date comes from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_source: Option<AgeSource>,
    /// Years since then.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_years: Option<f64>,
    /// Open PageRank, 0–10.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_page_rank: Option<f64>,
    /// Global Open PageRank position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_rank_position: Option<u64>,
    /// Referring domains.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referring_domains: Option<u64>,
    /// Source of the referring domains (`open-pagerank`, `dataforseo`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referring_source: Option<String>,
    /// Backlinks (DataForSEO).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backlinks: Option<u64>,
    /// Tranco rank.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tranco_rank: Option<u32>,
    /// Lookups that failed, by signal.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub errors: BTreeMap<String, String>,
}

/// Which lookups to make.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainOptions {
    /// Ignore cached facts.
    #[serde(default)]
    pub refresh: bool,
    /// Ask DataForSEO for backlink summaries (paid).
    #[serde(default)]
    pub backlinks: bool,
}

/// WHOIS over TCP.
struct NetWhois;

impl WhoisPort for NetWhois {
    fn query(&self, server: &str, query: &str) -> Result<String, String> {
        seonexus_net::whois(server, query, Duration::from_secs(15))
            .map_err(|error| error.to_string())
    }
}

fn now() -> Timestamp {
    Timestamp::now()
}

fn later(days: i64) -> String {
    Timestamp::from_unix_micros(now().as_unix_micros() + days * DAY_SECONDS * 1_000_000).to_string()
}

fn today() -> Option<Day> {
    Day::parse_iso(&now().to_string())
}

/// A cached value that has not expired.
///
/// Lookups go over the network, so the store is opened for each read and write only: other
/// work (the desktop app's views) can use it in between.
fn fresh(
    workspace: &SerpWorkspace,
    domain: &str,
    signal: &str,
    refresh: bool,
) -> Result<Option<DomainFact>, SerpAppError> {
    if refresh {
        return Ok(None);
    }
    Ok(workspace
        .store()?
        .domain_fact(domain, signal)?
        .filter(|fact| fact.expires_at.as_str() > now().to_string().as_str()))
}

fn remember(
    workspace: &SerpWorkspace,
    domain: &str,
    signal: &str,
    outcome: Result<(serde_json::Value, String), IntelError>,
    days: i64,
) -> Result<DomainFact, SerpAppError> {
    let fact = match outcome {
        Ok((value, source)) => DomainFact {
            domain: domain.to_owned(),
            signal: signal.to_owned(),
            value: Some(value.to_string()),
            source: Some(source),
            fetched_at: now().to_string(),
            expires_at: later(days),
            error: None,
        },
        Err(error) => DomainFact {
            domain: domain.to_owned(),
            signal: signal.to_owned(),
            value: None,
            source: None,
            fetched_at: now().to_string(),
            expires_at: later(
                if matches!(error, IntelError::RateLimited | IntelError::Network(_)) {
                    0
                } else {
                    FAILURE_DAYS
                },
            ),
            error: Some(error.to_string()),
        },
    };
    workspace.store()?.put_domain_fact(&fact)?;
    Ok(fact)
}

impl SerpWorkspace {
    /// Facts about domains (registrable domains of the hosts given), from the cache where it
    /// is fresh. A failed lookup is reported per domain and signal, not as an error.
    ///
    /// # Errors
    /// [`SerpAppError`] for storage failures.
    pub fn domain_facts(
        &self,
        hosts: &[String],
        options: DomainOptions,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<DomainFacts>, SerpAppError> {
        let settings = self.settings()?;
        let transport = self.transport(&settings)?;
        self.domain_facts_with(hosts, options, &transport, &NetWhois, cancelled)
    }

    /// [`Self::domain_facts`] through given network ports.
    ///
    /// # Errors
    /// As [`Self::domain_facts`].
    pub fn domain_facts_with(
        &self,
        hosts: &[String],
        options: DomainOptions,
        transport: &dyn HttpTransport,
        whois_port: &dyn WhoisPort,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<DomainFacts>, SerpAppError> {
        let mut domains: Vec<String> = hosts
            .iter()
            .map(|host| seonexus_domain_intel::registrable(host))
            .collect();
        domains.sort_unstable();
        domains.dedup();
        let mut facts: BTreeMap<String, DomainFacts> = domains
            .iter()
            .map(|domain| {
                (
                    domain.clone(),
                    DomainFacts {
                        domain: domain.clone(),
                        ..DomainFacts::default()
                    },
                )
            })
            .collect();
        // Addresses and local names have no registration data.
        let domains: Vec<String> = domains
            .into_iter()
            .filter(|domain| is_registered_name(domain))
            .collect();

        // Age, domain by domain; the RDAP bootstrap is fetched only when a lookup is due.
        let mut bootstrap: Option<Option<Bootstrap>> = None;
        for domain in &domains {
            if cancelled() {
                break;
            }
            let fact = if let Some(fact) = fresh(self, domain, "age", options.refresh)? {
                fact
            } else {
                let bootstrap = bootstrap.get_or_insert_with(|| self.rdap_bootstrap(transport));
                remember(
                    self,
                    domain,
                    "age",
                    lookup_age(transport, whois_port, bootstrap.as_ref(), domain),
                    AGE_DAYS,
                )?
            };
            let entry = facts.entry(domain.clone()).or_default();
            apply_age(entry, &fact);
        }

        self.pagerank_pass(&domains, options, transport, cancelled, &mut facts)?;
        if options.backlinks {
            self.backlinks_pass(&domains, options, transport, cancelled, &mut facts)?;
        }
        let store = self.store()?;
        for domain in &domains {
            if let Some(rank) = store.tranco_rank(domain)? {
                facts.entry(domain.clone()).or_default().tranco_rank = Some(rank);
            }
        }
        Ok(facts.into_values().collect())
    }

    fn pagerank_pass(
        &self,
        domains: &[String],
        options: DomainOptions,
        transport: &dyn HttpTransport,
        cancelled: &dyn Fn() -> bool,
        facts: &mut BTreeMap<String, DomainFacts>,
    ) -> Result<(), SerpAppError> {
        // Open PageRank, in batches, for domains without a fresh value.
        let origins = [DATA_SOURCES[0].2];
        let key = self.secrets().get("open-pagerank", "api_key", &origins)?;
        let stale: Vec<String> = domains
            .iter()
            .filter(|domain| {
                fresh(self, domain, "pagerank", options.refresh)
                    .ok()
                    .flatten()
                    .is_none()
            })
            .cloned()
            .collect();
        if let Some(key) = key.as_deref().filter(|_| !stale.is_empty() && !cancelled()) {
            match open_pagerank(transport, key, &stale) {
                Ok(ranks) => {
                    for domain in &stale {
                        let outcome = ranks.get(domain).map_or_else(
                            || Err(IntelError::NotFound(domain.clone())),
                            |rank| {
                                Ok((
                                    json!({"score": rank.score, "rank": rank.rank, "referring_domains": rank.referring_domains}),
                                    "open-pagerank".to_owned(),
                                ))
                            },
                        );
                        remember(self, domain, "pagerank", outcome, AUTHORITY_DAYS)?;
                    }
                }
                Err(error) => {
                    for domain in &stale {
                        facts
                            .entry(domain.clone())
                            .or_default()
                            .errors
                            .insert("pagerank".to_owned(), error.to_string());
                    }
                }
            }
        }
        let store = self.store()?;
        for domain in domains {
            let entry = facts.entry(domain.to_owned()).or_default();
            match store.domain_fact(domain, "pagerank")? {
                Some(fact) => apply_pagerank(entry, &fact),
                None if key.is_none() => {
                    entry.errors.insert(
                        "pagerank".to_owned(),
                        "set the Open PageRank API key to score authority".to_owned(),
                    );
                }
                None => {}
            }
        }

        Ok(())
    }

    fn backlinks_pass(
        &self,
        domains: &[String],
        options: DomainOptions,
        transport: &dyn HttpTransport,
        cancelled: &dyn Fn() -> bool,
        facts: &mut BTreeMap<String, DomainFacts>,
    ) -> Result<(), SerpAppError> {
        // The user's own login, or the placeholder when DataForSEO runs on OctopusOn SEO credits.
        let credentials = self.source_secret(
            &self.settings()?,
            "dataforseo",
            "credentials",
            "https://api.dataforseo.com",
        );
        for domain in domains {
            if cancelled() {
                break;
            }
            let fact = match (
                fresh(self, domain, "backlinks", options.refresh)?,
                credentials.as_deref(),
            ) {
                (Some(fact), _) => fact,
                (None, Some(credentials)) => {
                    let outcome = backlink_summary(transport, credentials, domain).map(|(summary, _)| {
                        (
                            json!({"backlinks": summary.backlinks, "referring_domains": summary.referring_domains, "rank": summary.rank}),
                            "dataforseo".to_owned(),
                        )
                    });
                    remember(self, domain, "backlinks", outcome, AUTHORITY_DAYS)?
                }
                (None, None) => {
                    facts.entry(domain.to_owned()).or_default().errors.insert(
                        "backlinks".to_owned(),
                        "set the DataForSEO credentials".to_owned(),
                    );
                    continue;
                }
            };
            apply_backlinks(facts.entry(domain.to_owned()).or_default(), &fact);
        }
        Ok(())
    }

    /// IANA's RDAP bootstrap file, cached for a month under `datasets/`.
    fn rdap_bootstrap(&self, transport: &dyn HttpTransport) -> Option<Bootstrap> {
        let path = self.data_dir().join("datasets").join("rdap-dns.json");
        let cached = fs::metadata(&path)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| {
                age.as_secs() < u64::try_from(BOOTSTRAP_DAYS * DAY_SECONDS).unwrap_or(u64::MAX)
            });
        if cached
            && let Some(bootstrap) = fs::read_to_string(&path)
                .ok()
                .and_then(|text| Bootstrap::parse(&text).ok())
        {
            return Some(bootstrap);
        }
        let response = transport
            .send(&HttpRequest::get(BOOTSTRAP_URL))
            .ok()
            .filter(seonexus_core::http::HttpResponse::is_success);
        match response
            .as_ref()
            .map(seonexus_core::http::HttpResponse::text)
        {
            Some(text) => {
                let bootstrap = Bootstrap::parse(&text).ok()?;
                let _ = write_atomically(&path, text.as_bytes());
                Some(bootstrap)
            }
            // Offline: an old copy is better than none.
            None => fs::read_to_string(&path)
                .ok()
                .and_then(|text| Bootstrap::parse(&text).ok()),
        }
    }

    /// Whether the Open PageRank key is set.
    ///
    /// # Errors
    /// [`SerpAppError`] when the bindings cannot be read.
    pub fn open_pagerank_state(&self) -> Result<SecretState, SerpAppError> {
        Ok(self
            .secrets()
            .state("open-pagerank", "api_key", &[DATA_SOURCES[0].2])?)
    }

    /// Downloads and imports the Tranco top-million list. The caller must have shown
    /// [`tranco::NOTICE`] and had the user accept it.
    ///
    /// # Errors
    /// [`SerpAppError`] for network, archive or storage failures.
    pub fn update_tranco(&self) -> Result<u64, SerpAppError> {
        let settings = self.settings()?;
        let client = seonexus_net::HttpClient::new(
            concat!("octopusSEO-SERP-Finder/", env!("CARGO_PKG_VERSION")),
            Duration::from_secs(settings.timeout_secs.max(120)),
        );
        let id = client
            .get_text(tranco::LIST_ID_URL)
            .ok()
            .map(|id| id.trim().to_owned());
        let archive = client
            .get_bytes_up_to(tranco::LIST_URL, 64 * 1024 * 1024)
            .map_err(|error| SerpAppError::Crawl(error.to_string()))?;
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive)).map_err(|error| {
            SerpAppError::InvalidFile {
                path: "top-1m.csv.zip".into(),
                message: error.to_string(),
            }
        })?;
        let mut csv = String::new();
        zip.by_index(0)
            .map_err(|error| SerpAppError::InvalidFile {
                path: "top-1m.csv.zip".into(),
                message: error.to_string(),
            })?
            .take(128 * 1024 * 1024)
            .read_to_string(&mut csv)
            .map_err(|source| SerpAppError::Io {
                path: "top-1m.csv".into(),
                source,
            })?;
        let path = self.data_dir().join("datasets").join("tranco-top-1m.csv");
        write_atomically(&path, csv.as_bytes())?;
        let rows = self
            .store()?
            .import_tranco(&path, id.as_deref(), &now().to_string())?;
        Ok(rows)
    }
}

/// Whether a host is a name under a public suffix (not an IP address or `localhost`).
fn is_registered_name(domain: &str) -> bool {
    domain.parse::<std::net::IpAddr>().is_err()
        && domain.contains('.')
        && psl::suffix_str(domain).is_some_and(|suffix| suffix != domain)
}

fn lookup_age(
    transport: &dyn HttpTransport,
    whois_port: &dyn WhoisPort,
    bootstrap: Option<&Bootstrap>,
    domain: &str,
) -> Result<(serde_json::Value, String), IntelError> {
    if let Some(record) = bootstrap
        .and_then(|bootstrap| rdap::lookup(transport, bootstrap, domain).ok())
        .filter(|record| record.registered.is_some())
    {
        return Ok((
            json!({"created": record.registered.map(|day| day.to_string()), "registrar": record.registrar}),
            "rdap".to_owned(),
        ));
    }
    let registry = match whois::lookup(whois_port, domain) {
        Ok(record) => {
            return Ok((
                json!({"created": record.created.map(|day| day.to_string()), "registrar": record.registrar}),
                "whois".to_owned(),
            ));
        }
        Err(error) => error,
    };
    match wayback::first_capture(transport, domain)? {
        Some(day) => Ok((json!({"created": day.to_string()}), "wayback".to_owned())),
        None => Err(registry),
    }
}

fn value(fact: &DomainFact) -> Option<serde_json::Value> {
    fact.value
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok())
}

fn apply_age(entry: &mut DomainFacts, fact: &DomainFact) {
    if let Some(error) = &fact.error {
        entry.errors.insert("age".to_owned(), error.clone());
        return;
    }
    let Some(created) = value(fact).and_then(|value| value["created"].as_str().map(str::to_owned))
    else {
        return;
    };
    entry.age_source = match fact.source.as_deref() {
        Some("rdap") => Some(AgeSource::Rdap),
        Some("whois") => Some(AgeSource::Whois),
        Some("wayback") => Some(AgeSource::Wayback),
        _ => None,
    };
    entry.age_years = Day::parse_iso(&created)
        .zip(today())
        .map(|(created, today)| (created.years_until(today) * 10.0).round() / 10.0);
    entry.created = Some(created);
}

fn apply_pagerank(entry: &mut DomainFacts, fact: &DomainFact) {
    if let Some(error) = &fact.error {
        entry.errors.insert("pagerank".to_owned(), error.clone());
        return;
    }
    let Some(value) = value(fact) else {
        return;
    };
    entry.open_page_rank = value["score"].as_f64();
    entry.page_rank_position = value["rank"].as_u64();
    if entry.referring_domains.is_none() {
        entry.referring_domains = value["referring_domains"].as_u64();
        entry.referring_source = entry.referring_domains.map(|_| "open-pagerank".to_owned());
    }
}

fn apply_backlinks(entry: &mut DomainFacts, fact: &DomainFact) {
    if let Some(error) = &fact.error {
        entry.errors.insert("backlinks".to_owned(), error.clone());
        return;
    }
    let Some(value) = value(fact) else {
        return;
    };
    entry.backlinks = value["backlinks"].as_u64();
    if let Some(referring) = value["referring_domains"].as_u64() {
        // Paid backlink data is more complete than the open link graph.
        entry.referring_domains = Some(referring);
        entry.referring_source = Some("dataforseo".to_owned());
    }
}

#[cfg(test)]
mod tests {
    use seonexus_core::http::testing::FakeTransport;

    use super::*;
    use crate::secrets::{FileBackend, Secrets};

    struct Whois;

    impl WhoisPort for Whois {
        fn query(&self, server: &str, query: &str) -> Result<String, String> {
            match (server, query) {
                ("whois.iana.org", "tr") => Ok("refer: whois.trabis.gov.tr\n".to_owned()),
                ("whois.trabis.gov.tr", _) => {
                    Ok("Created on..............: 2004-Feb-12.\n".to_owned())
                }
                _ => Ok(String::new()),
            }
        }
    }

    #[test]
    fn gathers_ages_and_authority_and_caches_them() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = SerpWorkspace::with_secrets(
            dir.path().to_owned(),
            Secrets::new(
                Box::new(FileBackend::new(dir.path().join("s.json"))),
                dir.path().join("b.json"),
            )
            .with_environment(|_| None),
        );
        workspace
            .set_secret("open-pagerank", "api_key", "opr-key")
            .unwrap();
        let bootstrap = r#"{"services": [[["com"], ["https://rdap.verisign.com/com/v1/"]]]}"#;
        let transport = FakeTransport::new()
            .reply(200, bootstrap)
            .reply(200, r#"{"objectClassName": "domain", "events": [{"eventAction": "registration", "eventDate": "1995-08-14T04:00:00Z"}]}"#)
            .reply(200, json!({"results": [
                {"domain": "example.com", "found": true, "open_page_rank": 7.5, "rank": 900, "referring_domains": 50_000},
                {"domain": "ornek.com.tr", "found": true, "open_page_rank": 3.1, "rank": 2_000_000, "referring_domains": 120}
            ]}).to_string());
        let hosts = vec![
            "www.example.com".to_owned(),
            "blog.ornek.com.tr".to_owned(),
            "example.com".to_owned(),
        ];
        let facts = workspace
            .domain_facts_with(
                &hosts,
                DomainOptions::default(),
                &transport,
                &Whois,
                &|| false,
            )
            .unwrap();
        assert_eq!(facts.len(), 2);
        let example = facts
            .iter()
            .find(|fact| fact.domain == "example.com")
            .unwrap();
        assert_eq!(example.created.as_deref(), Some("1995-08-14"));
        assert_eq!(example.age_source, Some(AgeSource::Rdap));
        assert!(example.age_years.unwrap() > 30.0);
        assert_eq!(example.open_page_rank, Some(7.5));
        let ornek = facts
            .iter()
            .find(|fact| fact.domain == "ornek.com.tr")
            .unwrap();
        assert_eq!(ornek.age_source, Some(AgeSource::Whois));
        assert_eq!(ornek.referring_domains, Some(120));
        assert_eq!(transport.requests().len(), 3);

        // Cached: no further requests.
        let again = FakeTransport::new();
        let cached = workspace
            .domain_facts_with(&hosts, DomainOptions::default(), &again, &Whois, &|| false)
            .unwrap();
        assert_eq!(cached, facts);
        assert_eq!(again.requests().len(), 0);
    }
}
