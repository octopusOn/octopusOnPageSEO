//! Sending a source's requests through the gateway.

use std::sync::{Mutex, PoisonError};

use seonexus_core::http::{HttpRequest, HttpResponse, HttpTransport, TransportError};

use crate::{CHARGE_HEADER, Connector};

/// Request headers passed to the gateway; the source's own key headers are dropped.
const KEPT_HEADERS: &[&str] = &["content-type", "accept", "accept-language", "user-agent"];

/// Whether `path` matches a gateway route pattern: `*` is one segment (or part of one), `**`
/// any number of segments.
pub fn matches_pattern(path: &str, pattern: &str) -> bool {
    let parts: Vec<&str> = path.split('/').skip(1).collect();
    let wanted: Vec<&str> = pattern.split('/').skip(1).collect();
    matches_from(&parts, &wanted)
}

fn matches_from(parts: &[&str], wanted: &[&str]) -> bool {
    match wanted.split_first() {
        None => parts.is_empty(),
        Some((&"**", rest)) => (0..=parts.len()).any(|skip| matches_from(&parts[skip..], rest)),
        Some((piece, rest)) => parts
            .split_first()
            .is_some_and(|(part, others)| glob(part, piece) && matches_from(others, rest)),
    }
}

/// One segment against a pattern segment where `*` matches any run of characters.
fn glob(segment: &str, piece: &str) -> bool {
    if piece == "*" {
        return !segment.is_empty();
    }
    let mut pieces = piece.split('*');
    let first = pieces.next().unwrap_or_default();
    let Some(mut rest) = segment.strip_prefix(first) else {
        return false;
    };
    let pieces: Vec<&str> = pieces.collect();
    let Some((last, middle)) = pieces.split_last() else {
        return rest.is_empty();
    };
    for piece in middle {
        match rest.find(piece) {
            Some(at) => rest = &rest[at + piece.len()..],
            None => return false,
        }
    }
    rest.ends_with(last)
}

/// Wraps a transport: requests to the upstream of a connector on credits go to the gateway
/// with the account token instead, and the charges the gateway reports are collected.
/// Everything else goes out unchanged.
pub struct CreditTransport<T> {
    inner: T,
    base: String,
    token: String,
    app: String,
    connectors: Vec<Connector>,
    charged: Mutex<Option<f64>>,
}

impl<T> std::fmt::Debug for CreditTransport<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreditTransport")
            .field("base", &self.base)
            .field(
                "connectors",
                &self
                    .connectors
                    .iter()
                    .map(|connector| &connector.id)
                    .collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl<T: HttpTransport> CreditTransport<T> {
    /// Sends the requests of `connectors` (those the user runs on credits) to the gateway at
    /// `base` with `token`; `app` names the program in the account's usage.
    pub fn new(
        inner: T,
        base: impl Into<String>,
        token: impl Into<String>,
        app: impl Into<String>,
        connectors: Vec<Connector>,
    ) -> Self {
        Self {
            inner,
            base: base.into(),
            token: token.into(),
            app: app.into(),
            connectors,
            charged: Mutex::new(None),
        }
    }

    /// The wrapped transport.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// The connector a request belongs to: same origin, and the route that matches when two
    /// connectors share an origin (Yandex Search API and Wordstat).
    fn connector(&self, request: &HttpRequest) -> Option<(&Connector, String)> {
        let url = url::Url::parse(&request.url).ok()?;
        let origin = url.origin().ascii_serialization();
        let mut rest = url.path().to_owned();
        if let Some(query) = url.query() {
            rest.push('?');
            rest.push_str(query);
        }
        let candidates: Vec<&Connector> = self
            .connectors
            .iter()
            .filter(|connector| connector.upstream.trim_end_matches('/') == origin)
            .collect();
        let routed = candidates.iter().find(|connector| {
            connector.routes.iter().any(|route| {
                route.method.eq_ignore_ascii_case(request.method.as_str())
                    && matches_pattern(url.path(), &route.path)
            })
        });
        routed
            .or_else(|| candidates.first())
            .map(|connector| (*connector, rest))
    }
}

impl<T: HttpTransport> HttpTransport for CreditTransport<T> {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let Some((connector, rest)) = self.connector(request) else {
            return self.inner.send(request);
        };
        let mut forwarded = HttpRequest {
            method: request.method,
            url: format!("{}/v1/p/{}{rest}", self.base, connector.id),
            headers: request
                .headers
                .iter()
                .filter(|(name, _)| {
                    KEPT_HEADERS
                        .iter()
                        .any(|kept| name.eq_ignore_ascii_case(kept))
                })
                .cloned()
                .collect(),
            body: request.body.clone(),
            timeout: request.timeout,
        };
        forwarded
            .headers
            .push(("authorization".to_owned(), format!("Bearer {}", self.token)));
        forwarded
            .headers
            .push(("x-octopus-app".to_owned(), self.app.clone()));
        let response = self.inner.send(&forwarded)?;
        if let Some(charge) = response
            .header(CHARGE_HEADER)
            .and_then(|value| value.trim().parse::<f64>().ok())
            .filter(|charge| charge.is_finite() && *charge >= 0.0)
        {
            let mut charged = self.charged.lock().unwrap_or_else(PoisonError::into_inner);
            *charged = Some(charged.unwrap_or(0.0) + charge);
        }
        Ok(response)
    }

    fn take_charge_usd(&self) -> Option<f64> {
        self.charged
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}
