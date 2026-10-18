//! RDAP (RFC 9082/9083): registration data in JSON, located through IANA's bootstrap file.

use std::collections::HashMap;

use seonexus_core::http::{HttpRequest, HttpTransport};
use serde_json::Value;

use crate::dates::Day;
use crate::{IntelError, top_level};

/// IANA's bootstrap file of RDAP services by top-level domain.
pub const BOOTSTRAP_URL: &str = "https://data.iana.org/rdap/dns.json";

/// RDAP base URLs by top-level domain.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bootstrap {
    services: HashMap<String, Vec<String>>,
}

impl Bootstrap {
    /// Reads IANA's `dns.json` (RFC 9224): `services` is a list of `[[tlds...], [urls...]]`.
    ///
    /// # Errors
    /// [`IntelError::Parse`] for JSON that is not a bootstrap file.
    pub fn parse(json: &str) -> Result<Self, IntelError> {
        let value: Value = serde_json::from_str(json)
            .map_err(|error| IntelError::Parse(format!("RDAP bootstrap: {error}")))?;
        let services = value
            .get("services")
            .and_then(Value::as_array)
            .ok_or_else(|| IntelError::Parse("RDAP bootstrap without services".to_owned()))?;
        let mut map = HashMap::new();
        for service in services {
            let (Some(tlds), Some(urls)) = (
                service.get(0).and_then(Value::as_array),
                service.get(1).and_then(Value::as_array),
            ) else {
                continue;
            };
            let mut urls: Vec<String> = urls
                .iter()
                .filter_map(Value::as_str)
                .map(|url| {
                    if url.ends_with('/') {
                        url.to_owned()
                    } else {
                        format!("{url}/")
                    }
                })
                .collect();
            // Prefer HTTPS servers.
            urls.sort_by_key(|url| !url.starts_with("https://"));
            for tld in tlds.iter().filter_map(Value::as_str) {
                map.insert(tld.to_ascii_lowercase(), urls.clone());
            }
        }
        Ok(Self { services: map })
    }

    /// The RDAP base URL serving a domain, if its TLD has one.
    pub fn base_for(&self, domain: &str) -> Option<&str> {
        self.services
            .get(top_level(domain))
            .and_then(|urls| urls.first())
            .map(String::as_str)
    }

    /// Number of TLDs covered.
    pub fn len(&self) -> usize {
        self.services.len()
    }

    /// Whether no TLD is covered.
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }
}

/// What an RDAP domain record says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RdapRecord {
    /// Registration date.
    pub registered: Option<Day>,
    /// Expiration date.
    pub expires: Option<Day>,
    /// Last change.
    pub changed: Option<Day>,
    /// Registrar name.
    pub registrar: Option<String>,
}

/// Reads a domain object (RFC 9083 §5.3).
///
/// # Errors
/// [`IntelError::Parse`] for JSON that is not a domain object.
pub fn parse_domain(json: &str) -> Result<RdapRecord, IntelError> {
    let value: Value =
        serde_json::from_str(json).map_err(|error| IntelError::Parse(format!("RDAP: {error}")))?;
    if value.get("objectClassName").and_then(Value::as_str) != Some("domain") {
        return Err(IntelError::Parse(
            "the RDAP answer is not a domain object".to_owned(),
        ));
    }
    let event = |action: &str| {
        value
            .get("events")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|event| event.get("eventAction").and_then(Value::as_str) == Some(action))
            .and_then(|event| event.get("eventDate").and_then(Value::as_str))
            .and_then(Day::parse_iso)
    };
    let registrar = value
        .get("entities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|entity| {
            entity
                .get("roles")
                .and_then(Value::as_array)
                .is_some_and(|roles| roles.iter().any(|role| role == "registrar"))
        })
        .and_then(vcard_name);
    Ok(RdapRecord {
        registered: event("registration"),
        expires: event("expiration"),
        changed: event("last changed"),
        registrar,
    })
}

/// The `fn` of an entity's jCard (RFC 7095).
fn vcard_name(entity: &Value) -> Option<String> {
    entity
        .get("vcardArray")?
        .get(1)?
        .as_array()?
        .iter()
        .find(|property| property.get(0).and_then(Value::as_str) == Some("fn"))?
        .get(3)?
        .as_str()
        .map(str::to_owned)
}

/// Looks a domain up over RDAP.
///
/// # Errors
/// [`IntelError::Unsupported`] when the TLD has no RDAP service; [`IntelError::Network`],
/// [`IntelError::NotFound`] or [`IntelError::Parse`] otherwise.
pub fn lookup(
    transport: &dyn HttpTransport,
    bootstrap: &Bootstrap,
    domain: &str,
) -> Result<RdapRecord, IntelError> {
    let base = bootstrap.base_for(domain).ok_or_else(|| {
        IntelError::Unsupported(format!("no RDAP service for .{}", top_level(domain)))
    })?;
    let request = HttpRequest::get(format!("{base}domain/{domain}"))
        .header("Accept", "application/rdap+json");
    let response = transport
        .send(&request)
        .map_err(|error| IntelError::Network(error.to_string()))?;
    match response.status {
        200 => parse_domain(&response.text()),
        404 => Err(IntelError::NotFound(domain.to_owned())),
        429 => Err(IntelError::RateLimited),
        status => Err(IntelError::Network(format!("RDAP answered HTTP {status}"))),
    }
}

#[cfg(test)]
mod tests {
    use seonexus_core::http::testing::FakeTransport;

    use super::*;

    const BOOTSTRAP: &str = r#"{"version": "1.0", "publication": "2026-09-30T23:00:03Z", "services": [
        [["com", "net"], ["https://rdap.verisign.com/com/v1/"]],
        [["org"], ["http://rdap.example/org", "https://rdap.publicinterestregistry.org/rdap"]]
    ]}"#;

    const DOMAIN: &str = r#"{"objectClassName": "domain", "ldhName": "EXAMPLE.COM",
        "events": [
            {"eventAction": "registration", "eventDate": "1995-08-14T04:00:00Z"},
            {"eventAction": "expiration", "eventDate": "2027-08-13T04:00:00Z"},
            {"eventAction": "last changed", "eventDate": "2026-08-14T07:01:39Z"}
        ],
        "entities": [{"objectClassName": "entity", "roles": ["registrar"],
            "vcardArray": ["vcard", [["version", {}, "text", "4.0"], ["fn", {}, "text", "RESERVED-Internet Assigned Numbers Authority"]]]}]}"#;

    #[test]
    fn reads_the_bootstrap_file_and_domain_records() {
        let bootstrap = Bootstrap::parse(BOOTSTRAP).unwrap();
        assert_eq!(bootstrap.len(), 3);
        assert_eq!(
            bootstrap.base_for("example.com"),
            Some("https://rdap.verisign.com/com/v1/")
        );
        assert_eq!(
            bootstrap.base_for("example.org"),
            Some("https://rdap.publicinterestregistry.org/rdap/")
        );
        assert_eq!(bootstrap.base_for("example.com.tr"), None);

        let record = parse_domain(DOMAIN).unwrap();
        assert_eq!(
            record.registered.map(|day| day.to_string()).as_deref(),
            Some("1995-08-14")
        );
        assert_eq!(
            record.expires.map(|day| day.to_string()).as_deref(),
            Some("2027-08-13")
        );
        assert!(record.registrar.unwrap().contains("Assigned Numbers"));
        assert!(parse_domain(r#"{"objectClassName": "entity"}"#).is_err());
    }

    #[test]
    fn looks_domains_up() {
        let bootstrap = Bootstrap::parse(BOOTSTRAP).unwrap();
        let transport = FakeTransport::new().reply(200, DOMAIN).reply(404, "");
        let record = lookup(&transport, &bootstrap, "example.com").unwrap();
        assert!(record.registered.is_some());
        assert_eq!(
            transport.requests()[0].url,
            "https://rdap.verisign.com/com/v1/domain/example.com"
        );
        assert!(matches!(
            lookup(&transport, &bootstrap, "missing.com"),
            Err(IntelError::NotFound(_))
        ));
        assert!(matches!(
            lookup(&transport, &bootstrap, "x.com.tr"),
            Err(IntelError::Unsupported(_))
        ));
    }
}
