//! How engines are told which country, language, place and page a search is for.
//!
//! [`search_url`] builds the address a browser opens for one result page. The parameters are
//! the public ones each engine documents or shows in its own links: Google `hl`, `gl`, `start`
//! and `uule`; Bing `setlang`, `cc`, `mkt` and `first`; Yandex `lr`, `lang` and `p` on the
//! country's Yandex domain. All three serve ten results per page to a browser: Google ignores
//! `num` since September 2025, and Bing `count=50` and Yandex `numdoc=50` were ignored in a
//! live check on 2026-10-18, so deeper searches open more pages.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use url::Url;

use crate::{Engine, RESULTS_PER_PAGE, SerpError, SerpQuery};

/// Address of result page `page` (from 1) of `query` as a browser would open it.
///
/// # Errors
/// [`SerpError::InvalidQuery`] when the query does not validate or `page` is 0.
pub fn search_url(query: &SerpQuery, page: u16) -> Result<Url, SerpError> {
    query.validate()?;
    if page == 0 {
        return Err(SerpError::InvalidQuery(
            "pages are counted from 1".to_owned(),
        ));
    }
    let keyword = query.keyword.trim();
    let offset = (page - 1) * RESULTS_PER_PAGE;
    let mut params: Vec<(&str, String)> = Vec::new();
    let base = match query.engine {
        Engine::Google => {
            params.push(("q", keyword.to_owned()));
            params.push(("hl", query.language.clone()));
            params.push(("gl", query.country.clone()));
            if offset > 0 {
                params.push(("start", offset.to_string()));
            }
            params.push(("pws", "0".to_owned()));
            if let Some(uule) = query
                .location
                .as_ref()
                .and_then(|location| location.google_canonical.as_deref())
                .and_then(uule)
            {
                params.push(("uule", uule));
            }
            "https://www.google.com/search"
        }
        Engine::Bing => {
            params.push(("q", keyword.to_owned()));
            if keyword.contains("site:") {
                params.push(("norw", "1".to_owned()));
            }
            params.push(("setlang", query.language.clone()));
            params.push(("cc", query.country.clone()));
            params.push(("mkt", bing_market(&query.language, &query.country)));
            if offset > 0 {
                params.push(("first", (offset + 1).to_string()));
            }
            "https://www.bing.com/search"
        }
        Engine::Yandex => {
            params.push(("text", keyword.to_owned()));
            if let Some(region) = yandex_region(query) {
                params.push(("lr", region.to_string()));
            }
            params.push(("lang", query.language.clone()));
            if page > 1 {
                params.push(("p", (page - 1).to_string()));
            }
            yandex_search_base(&query.country)
        }
    };
    Url::parse_with_params(base, &params)
        .map_err(|error| SerpError::InvalidQuery(error.to_string()))
}

/// Google's `uule` value for a location's canonical name (`Istanbul,Istanbul,Turkey`):
/// `w+CAIQICI`, a character encoding the name's length, then the name in base64.
///
/// `None` for an empty name or one longer than 63 bytes, which the scheme cannot encode.
pub fn uule(canonical_name: &str) -> Option<String> {
    const LENGTH_KEY: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let name = canonical_name.trim();
    let key = LENGTH_KEY.get(name.len()).filter(|_| !name.is_empty())?;
    Some(format!(
        "w+CAIQICI{}{}",
        char::from(*key),
        STANDARD.encode(name)
    ))
}

/// Bing market code (`tr-TR`).
pub fn bing_market(language: &str, country: &str) -> String {
    format!(
        "{}-{}",
        language.to_ascii_lowercase(),
        country.to_ascii_uppercase()
    )
}

/// The Yandex site serving a country: `yandex.com.tr` for Turkey, the national domain for
/// Russia, Kazakhstan, Belarus and Uzbekistan, `yandex.com` elsewhere.
pub fn yandex_search_base(country: &str) -> &'static str {
    match country {
        "tr" => "https://yandex.com.tr/search/",
        "ru" => "https://yandex.ru/search/",
        "kz" => "https://yandex.kz/search/",
        "by" => "https://yandex.by/search/",
        "uz" => "https://yandex.uz/search/",
        _ => "https://yandex.com/search/",
    }
}

/// English name of a country, for providers that take names (`tr` → `Turkey`).
pub fn country_name(country: &str) -> Option<&'static str> {
    COUNTRY_NAMES
        .iter()
        .find(|(code, _)| *code == country)
        .map(|(_, name)| *name)
}

/// Countries agencies work with most, by ISO code.
const COUNTRY_NAMES: &[(&str, &str)] = &[
    ("tr", "Turkey"),
    ("az", "Azerbaijan"),
    ("cy", "Cyprus"),
    ("us", "United States"),
    ("gb", "United Kingdom"),
    ("ie", "Ireland"),
    ("ca", "Canada"),
    ("au", "Australia"),
    ("de", "Germany"),
    ("at", "Austria"),
    ("ch", "Switzerland"),
    ("nl", "Netherlands"),
    ("be", "Belgium"),
    ("fr", "France"),
    ("es", "Spain"),
    ("pt", "Portugal"),
    ("it", "Italy"),
    ("gr", "Greece"),
    ("bg", "Bulgaria"),
    ("ro", "Romania"),
    ("pl", "Poland"),
    ("se", "Sweden"),
    ("no", "Norway"),
    ("dk", "Denmark"),
    ("fi", "Finland"),
    ("ru", "Russia"),
    ("ua", "Ukraine"),
    ("kz", "Kazakhstan"),
    ("uz", "Uzbekistan"),
    ("by", "Belarus"),
    ("ge", "Georgia"),
    ("ae", "United Arab Emirates"),
    ("sa", "Saudi Arabia"),
    ("qa", "Qatar"),
    ("eg", "Egypt"),
    ("il", "Israel"),
    ("in", "India"),
    ("br", "Brazil"),
    ("mx", "Mexico"),
    ("jp", "Japan"),
];

/// Yandex Search API `searchType` for a country.
pub fn yandex_search_type(country: &str) -> &'static str {
    match country {
        "tr" => "SEARCH_TYPE_TR",
        "ru" => "SEARCH_TYPE_RU",
        "kz" => "SEARCH_TYPE_KK",
        "by" => "SEARCH_TYPE_BE",
        "uz" => "SEARCH_TYPE_UZ",
        _ => "SEARCH_TYPE_COM",
    }
}

/// Yandex Search API `l10n` (interface language) for a language code.
pub fn yandex_localization(language: &str) -> &'static str {
    match language {
        "tr" => "LOCALIZATION_TR",
        "ru" => "LOCALIZATION_RU",
        "uk" => "LOCALIZATION_UK",
        "be" => "LOCALIZATION_BE",
        "kk" => "LOCALIZATION_KK",
        _ => "LOCALIZATION_EN",
    }
}

/// Yandex region id of the query: the location's, else the country's.
pub fn yandex_region(query: &SerpQuery) -> Option<u32> {
    query
        .location
        .as_ref()
        .and_then(|location| location.yandex_lr)
        .or_else(|| yandex_country_region(&query.country))
}

/// Yandex region id of a whole country.
pub fn yandex_country_region(country: &str) -> Option<u32> {
    YANDEX_REGIONS
        .iter()
        .find(|region| region.country == country && region.whole_country)
        .map(|region| region.id)
}

/// A Yandex region id with its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YandexRegion {
    /// Region id (`lr`).
    pub id: u32,
    /// English name.
    pub name: &'static str,
    /// Country code.
    pub country: &'static str,
    /// Whether the region is the whole country.
    pub whole_country: bool,
}

/// Countries and the largest Turkish cities, for pickers; any other id can be typed in.
pub const YANDEX_REGIONS: &[YandexRegion] = &[
    region(983, "Turkey", "tr", true),
    region(11508, "Istanbul", "tr", false),
    region(11503, "Ankara", "tr", false),
    region(11505, "Izmir", "tr", false),
    region(11511, "Antalya", "tr", false),
    region(225, "Russia", "ru", true),
    region(213, "Moscow", "ru", false),
    region(2, "Saint Petersburg", "ru", false),
    region(159, "Kazakhstan", "kz", true),
    region(149, "Belarus", "by", true),
    region(171, "Uzbekistan", "uz", true),
    region(187, "Ukraine", "ua", true),
    region(96, "Germany", "de", true),
    region(84, "United States", "us", true),
];

const fn region(
    id: u32,
    name: &'static str,
    country: &'static str,
    whole_country: bool,
) -> YandexRegion {
    YandexRegion {
        id,
        name,
        country,
        whole_country,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Location, SerpQuery};

    #[test]
    fn encodes_uule_like_google() {
        assert_eq!(
            uule("Paris,Ile-de-France,France").as_deref(),
            Some("w+CAIQICIaUGFyaXMsSWxlLWRlLUZyYW5jZSxGcmFuY2U=")
        );
        assert_eq!(uule(""), None);
        assert_eq!(uule(&"x".repeat(64)), None);
    }

    #[test]
    fn builds_google_urls() {
        let mut query = SerpQuery::new(Engine::Google, "seo ajansı").with_depth(20);
        query.location = Some(Location {
            google_canonical: Some("Istanbul,Istanbul,Turkey".to_owned()),
            ..Location::default()
        });
        let first = search_url(&query, 1).unwrap();
        assert_eq!(first.host_str(), Some("www.google.com"));
        let pairs: Vec<(String, String)> = first.query_pairs().into_owned().collect();
        assert!(pairs.contains(&("q".to_owned(), "seo ajansı".to_owned())));
        assert!(pairs.contains(&("gl".to_owned(), "tr".to_owned())));
        assert!(pairs.iter().any(|(key, _)| key == "uule"));
        assert!(!pairs.iter().any(|(key, _)| key == "start"));
        let second = search_url(&query, 2).unwrap();
        assert!(
            second
                .query_pairs()
                .any(|(key, value)| key == "start" && value == "10")
        );
    }

    #[test]
    fn builds_bing_and_yandex_urls() {
        let bing = search_url(&SerpQuery::new(Engine::Bing, "kahve"), 2).unwrap();
        let pairs: Vec<(String, String)> = bing.query_pairs().into_owned().collect();
        assert!(pairs.contains(&("mkt".to_owned(), "tr-TR".to_owned())));
        assert!(pairs.contains(&("first".to_owned(), "11".to_owned())));

        let yandex = search_url(&SerpQuery::new(Engine::Yandex, "kahve"), 3).unwrap();
        assert_eq!(yandex.host_str(), Some("yandex.com.tr"));
        let pairs: Vec<(String, String)> = yandex.query_pairs().into_owned().collect();
        assert!(pairs.contains(&("lr".to_owned(), "983".to_owned())));
        assert!(pairs.contains(&("p".to_owned(), "2".to_owned())));
    }

    #[test]
    fn names_countries() {
        assert_eq!(country_name("tr"), Some("Turkey"));
        assert_eq!(country_name("xx"), None);
    }

    #[test]
    fn rejects_page_zero_and_invalid_queries() {
        assert!(search_url(&SerpQuery::new(Engine::Google, "x"), 0).is_err());
        assert!(search_url(&SerpQuery::new(Engine::Google, "  "), 1).is_err());
        assert!(
            search_url(
                &SerpQuery::new(Engine::Google, "x").localised("tur", "tr"),
                1
            )
            .is_err()
        );
    }
}
