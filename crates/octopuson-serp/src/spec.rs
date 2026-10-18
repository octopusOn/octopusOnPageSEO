//! Providers users define themselves: a request template and a response mapping.
//!
//! ```json
//! {"version": 1, "id": "acme", "name": "Acme SERP", "engines": ["google"],
//!  "secrets": [{"name": "token"}],
//!  "request": {"method": "POST", "url": "https://api.acme.example/v1/search?page={page}",
//!    "headers": {"Authorization": "Bearer {secret:token}"},
//!    "body": {"q": "{keyword}", "gl": "{gl}", "hl": "{hl}", "num": "{num:int}"}},
//!  "paging": {"per_page": 10, "first_page": 1, "max_pages": 2},
//!  "response": {"format": "json", "error": "$.error.message", "total": "$.info.total",
//!    "results": {"items": "$.organic[*]",
//!      "fields": {"url": "$.link", "title": "$.title", "snippet": "$.snippet"}},
//!    "features": [{"kind": "paa", "items": "$.related_questions[*]",
//!      "fields": {"text": "$.question"}}]},
//!  "cost": {"per_request_usd": 0.002}, "rate_limit": {"per_minute": 60}}
//! ```
//!
//! The scheme, host and port of the URL must be written out: secrets are bound to that origin
//! when they are saved, so a template cannot send them anywhere a placeholder decides.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::mapping::{CompiledMapping, Format, Mapping, ParsedPage, Probe, Selectors, json_text};
use crate::profile::merge_patch;
use crate::template::{self, Renderer, TemplateContext, is_secret_name};
use crate::{Device, Engine, RESULTS_PER_PAGE, SerpError, SerpQuery};

/// Version of the spec format.
pub const SPEC_VERSION: u32 = 1;

/// HTTP method of a provider request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    /// GET without a body.
    #[default]
    Get,
    /// POST with the body.
    Post,
}

impl Method {
    /// Upper-case name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}

/// A secret the provider needs (an API key, a token).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretSpec {
    /// Name used in `{secret:<name>}`.
    pub name: String,
    /// Label shown when asking for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// The request of one result page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestTemplate {
    /// HTTP method.
    #[serde(default)]
    pub method: Method,
    /// URL template; scheme, host and port must be literal.
    pub url: String,
    /// Query parameters appended to the URL; a parameter whose value is a single optional
    /// placeholder without a value (`"location": "{location_name}"`) is left out.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub query: BTreeMap<String, String>,
    /// Header templates.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// Body template: a JSON tree sent as JSON, or a string sent as it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

/// How results are paged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paging {
    /// Results per request.
    #[serde(default = "per_page")]
    pub per_page: u16,
    /// Number the provider gives its first page.
    #[serde(default = "first_page")]
    pub first_page: u32,
    /// Most requests per query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_pages: Option<u16>,
}

impl Default for Paging {
    fn default() -> Self {
        Self {
            per_page: per_page(),
            first_page: first_page(),
            max_pages: None,
        }
    }
}

const fn per_page() -> u16 {
    RESULTS_PER_PAGE
}

const fn first_page() -> u32 {
    1
}

/// Price of a provider's requests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct CostSpec {
    /// US dollars per request, for estimates.
    #[serde(default)]
    pub per_request_usd: f64,
    /// US dollars per credit, to convert credits a response reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_credit_usd: Option<f64>,
}

/// Changes of the request and response for one engine, as JSON merge patches (RFC 7396).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EngineOverride {
    /// Patch of `request`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<Value>,
    /// Patch of `response`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<Value>,
}

/// A status code inside successful JSON responses (APIs that answer 200 with an error code).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusSpec {
    /// `JSONPath` or JSON Pointer of the code, or alternatives tried in order.
    pub path: Selectors,
    /// Where the message explaining the code is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<Selectors>,
    /// Codes meaning success; anything else is an error.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ok: Vec<String>,
    /// Codes meaning invalid credentials.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auth: Vec<String>,
    /// Codes meaning an exhausted balance or quota.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quota: Vec<String>,
    /// Codes meaning too many requests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rate_limited: Vec<String>,
}

/// Where a JSON response reports what the request cost.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageSpec {
    /// US dollars charged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<String>,
    /// Credits charged (converted with `cost.per_credit_usd`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<String>,
}

/// Request rate the provider allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitSpec {
    /// Requests per minute.
    pub per_minute: u32,
}

/// A user-defined SERP provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderSpec {
    /// Spec format version.
    #[serde(default = "spec_version")]
    pub version: u32,
    /// Identifier: 1–40 of `a-z`, `0-9` and `-`, starting with a letter or digit.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Engines whose results it returns.
    pub engines: Vec<Engine>,
    /// Devices it can return results for; empty for both.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<Device>,
    /// Secrets it needs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<SecretSpec>,
    /// The request of one page.
    pub request: RequestTemplate,
    /// Paging.
    #[serde(default)]
    pub paging: Paging,
    /// Where the results are in a response.
    pub response: Mapping,
    /// Price, for budgets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<CostSpec>,
    /// Rate limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitSpec>,
    /// Per-engine changes of request and response.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub engine_overrides: BTreeMap<Engine, EngineOverride>,
    /// Error codes inside successful JSON responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<StatusSpec>,
    /// Cost a JSON response reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageSpec>,
    /// Free-form notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

const fn spec_version() -> u32 {
    SPEC_VERSION
}

/// A request ready to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedRequest {
    /// Method.
    pub method: Method,
    /// Absolute URL.
    pub url: Url,
    /// Headers in order.
    pub headers: Vec<(String, String)>,
    /// Body.
    pub body: Option<Vec<u8>>,
    secret_values: Vec<String>,
}

impl RenderedRequest {
    /// The URL with secret values replaced by `***`, for logs and stored snapshots.
    pub fn redacted_url(&self) -> String {
        template::redact(self.url.as_str(), &self.secret_values)
    }

    /// `text` with this request's secret values replaced by `***`.
    pub fn redact(&self, text: &str) -> String {
        template::redact(text, &self.secret_values)
    }
}

/// What a status code inside a response means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusOutcome {
    /// Success (or no status rule).
    Ok,
    /// Invalid credentials.
    Auth(String),
    /// Exhausted balance or quota.
    Quota(String),
    /// Too many requests.
    RateLimited(String),
    /// Another error code.
    Error(String),
}

/// What a response reports it cost.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ReportedUsage {
    /// US dollars, reported or converted from credits.
    pub cost_usd: Option<f64>,
    /// Credits reported.
    pub credits: Option<f64>,
}

/// One engine's request and mapping.
#[derive(Debug, Clone)]
struct Variant {
    request: RequestTemplate,
    mapping: CompiledMapping,
    origin: String,
}

/// A validated spec with its mappings compiled.
#[derive(Debug, Clone)]
pub struct CompiledSpec {
    spec: ProviderSpec,
    variants: BTreeMap<Engine, Variant>,
    status: Option<Vec<Probe>>,
    status_message: Vec<Probe>,
    usage_cost: Vec<Probe>,
    usage_credits: Vec<Probe>,
}

impl ProviderSpec {
    /// Validates the spec and compiles its mappings.
    ///
    /// # Errors
    /// [`SerpError::InvalidSpec`] or [`SerpError::Template`] naming the first problem.
    pub fn compile(&self) -> Result<CompiledSpec, SerpError> {
        self.validate_metadata()?;
        let declared: Vec<&str> = self
            .secrets
            .iter()
            .map(|secret| secret.name.as_str())
            .collect();
        if let Some(engine) = self
            .engine_overrides
            .keys()
            .find(|engine| !self.engines.contains(engine))
        {
            return Err(invalid(format!(
                "`engine_overrides` names {engine}, which is not in `engines`"
            )));
        }
        let mut variants = BTreeMap::new();
        for engine in &self.engines {
            let (request, response) = self.variant_parts(*engine)?;
            let variant = compile_variant(&request, &response, &declared)
                .map_err(|error| prefix_engine(error, *engine, self.engines.len()))?;
            variants.insert(*engine, variant);
        }
        let json_probes = |paths: Vec<&str>| -> Result<Vec<Probe>, SerpError> {
            paths
                .into_iter()
                .map(|path| Probe::compile(Format::Json, path))
                .collect()
        };
        let status = self
            .status
            .as_ref()
            .map(|status| json_probes(status.path.as_list()))
            .transpose()?;
        let status_message = json_probes(
            self.status
                .as_ref()
                .and_then(|status| status.message.as_ref())
                .map(Selectors::as_list)
                .unwrap_or_default(),
        )?;
        let usage = self.usage.clone().unwrap_or_default();
        Ok(CompiledSpec {
            spec: self.clone(),
            variants,
            status,
            status_message,
            usage_cost: json_probes(usage.cost_usd.as_deref().into_iter().collect())?,
            usage_credits: json_probes(usage.credits.as_deref().into_iter().collect())?,
        })
    }

    fn validate_metadata(&self) -> Result<(), SerpError> {
        if self.version != SPEC_VERSION {
            return Err(invalid(format!(
                "unsupported spec version {} (expected {SPEC_VERSION})",
                self.version
            )));
        }
        if !is_provider_id(&self.id) {
            return Err(invalid(format!(
                "invalid id `{}`: use 1–40 of a-z, 0-9 and -, starting with a letter or digit",
                self.id
            )));
        }
        let name_length = self.name.trim().chars().count();
        if name_length == 0 || name_length > 80 {
            return Err(invalid("the name must have 1–80 characters".to_owned()));
        }
        if self.engines.is_empty() {
            return Err(invalid("name at least one engine".to_owned()));
        }
        let mut engines = self.engines.clone();
        engines.sort_unstable();
        engines.dedup();
        if engines.len() != self.engines.len() {
            return Err(invalid("an engine is listed twice".to_owned()));
        }
        let mut declared = Vec::new();
        for secret in &self.secrets {
            if !is_secret_name(&secret.name) {
                return Err(invalid(format!(
                    "invalid secret name `{}` (use a-z, 0-9 and _)",
                    secret.name
                )));
            }
            if declared.contains(&secret.name.as_str()) {
                return Err(invalid(format!(
                    "secret `{}` is declared twice",
                    secret.name
                )));
            }
            declared.push(secret.name.as_str());
        }
        if !(1..=100).contains(&self.paging.per_page) {
            return Err(invalid(
                "paging.per_page must be between 1 and 100".to_owned(),
            ));
        }
        if self.paging.max_pages == Some(0) {
            return Err(invalid("paging.max_pages must be at least 1".to_owned()));
        }
        if let Some(cost) = self.cost {
            let valid = |value: f64| value.is_finite() && value >= 0.0;
            if !valid(cost.per_request_usd) || !cost.per_credit_usd.is_none_or(valid) {
                return Err(invalid("costs must be zero or more".to_owned()));
            }
        }
        if self.rate_limit.is_some_and(|limit| limit.per_minute == 0) {
            return Err(invalid(
                "rate_limit.per_minute must be at least 1".to_owned(),
            ));
        }
        Ok(())
    }

    /// Request and response of one engine after its override.
    fn variant_parts(&self, engine: Engine) -> Result<(RequestTemplate, Mapping), SerpError> {
        let Some(patch) = self.engine_overrides.get(&engine) else {
            return Ok((self.request.clone(), self.response.clone()));
        };
        let patched = |base: Value, patch: Option<&Value>| {
            let mut value = base;
            if let Some(patch) = patch {
                merge_patch(&mut value, patch);
            }
            value
        };
        let to_value = |value: Result<Value, serde_json::Error>| {
            value.map_err(|error| invalid(error.to_string()))
        };
        let request = patched(
            to_value(serde_json::to_value(&self.request))?,
            patch.request.as_ref(),
        );
        let response = patched(
            to_value(serde_json::to_value(&self.response))?,
            patch.response.as_ref(),
        );
        let request = serde_json::from_value(request)
            .map_err(|error| invalid(format!("{engine} request override: {error}")))?;
        let response = serde_json::from_value(response)
            .map_err(|error| invalid(format!("{engine} response override: {error}")))?;
        Ok((request, response))
    }
}

fn compile_variant(
    request: &RequestTemplate,
    response: &Mapping,
    declared: &[&str],
) -> Result<Variant, SerpError> {
    if request.method == Method::Get && request.body.is_some() {
        return Err(invalid("a GET request has no body; use POST".to_owned()));
    }
    let mut used = template::usage(&request.url)?;
    for (name, value) in &request.query {
        if name.trim().is_empty() {
            return Err(invalid("a query parameter needs a name".to_owned()));
        }
        used.secrets.extend(template::usage(value)?.secrets);
    }
    for (name, value) in &request.headers {
        if !is_header_name(name) {
            return Err(invalid(format!("invalid header name `{name}`")));
        }
        used.secrets.extend(template::usage(value)?.secrets);
    }
    if let Some(body) = &request.body {
        used.secrets.extend(template::body_usage(body)?.secrets);
    }
    if let Some(missing) = used
        .secrets
        .iter()
        .find(|name| !declared.contains(&name.as_str()))
    {
        return Err(invalid(format!(
            "`{{secret:{missing}}}` is used but not declared in `secrets`"
        )));
    }
    Ok(Variant {
        origin: literal_origin(&request.url)?,
        mapping: response.compile()?,
        request: request.clone(),
    })
}

fn prefix_engine(error: SerpError, engine: Engine, engines: usize) -> SerpError {
    if engines < 2 {
        return error;
    }
    match error {
        SerpError::InvalidSpec(message) => SerpError::InvalidSpec(format!("{engine}: {message}")),
        SerpError::Template(message) => SerpError::Template(format!("{engine}: {message}")),
        other => other,
    }
}

impl CompiledSpec {
    /// The spec.
    pub fn spec(&self) -> &ProviderSpec {
        &self.spec
    }

    /// The compiled response mapping of an engine.
    pub fn mapping(&self, engine: Engine) -> Option<&CompiledMapping> {
        self.variants.get(&engine).map(|variant| &variant.mapping)
    }

    /// `scheme://host[:port]` an engine's requests go to.
    pub fn origin(&self, engine: Engine) -> Option<&str> {
        self.variants
            .get(&engine)
            .map(|variant| variant.origin.as_str())
    }

    /// Every origin requests go to; secrets are bound to these.
    pub fn origins(&self) -> Vec<&str> {
        let mut origins: Vec<&str> = self
            .variants
            .values()
            .map(|variant| variant.origin.as_str())
            .collect();
        origins.sort_unstable();
        origins.dedup();
        origins
    }

    /// `id@v<version>`, recorded with snapshots.
    pub fn revision(&self) -> String {
        format!("{}@v{}", self.spec.id, self.spec.version)
    }

    /// Requests a query takes.
    pub fn pages(&self, query: &SerpQuery) -> u16 {
        let wanted = query
            .depth
            .clamp(1, crate::MAX_DEPTH)
            .div_ceil(self.spec.paging.per_page);
        self.spec
            .paging
            .max_pages
            .map_or(wanted, |max| wanted.min(max))
            .max(1)
    }

    /// Whether the provider returns results for the query's engine and device.
    pub fn supports(&self, query: &SerpQuery) -> bool {
        self.variants.contains_key(&query.engine)
            && (self.spec.devices.is_empty() || self.spec.devices.contains(&query.device))
    }

    /// Estimated price of a query in US dollars.
    pub fn estimate_usd(&self, query: &SerpQuery) -> f64 {
        self.spec.cost.map_or(0.0, |cost| {
            cost.per_request_usd * f64::from(self.pages(query))
        })
    }

    /// Builds the request of result page `page` (from 1).
    ///
    /// # Errors
    /// [`SerpError::Template`] for missing secrets or values that cannot be filled in;
    /// [`SerpError::InvalidSpec`] when the engine is not supported or the URL is not an
    /// absolute http(s) URL on the spec's origin.
    pub fn render(
        &self,
        query: &SerpQuery,
        page: u16,
        secrets: &dyn Fn(&str) -> Option<String>,
    ) -> Result<RenderedRequest, SerpError> {
        query.validate()?;
        let variant = self.variants.get(&query.engine).ok_or_else(|| {
            invalid(format!(
                "`{}` does not search {}",
                self.spec.id, query.engine
            ))
        })?;
        let context = TemplateContext {
            query,
            page,
            per_page: self.spec.paging.per_page,
            first_page: self.spec.paging.first_page,
        };
        let mut renderer = Renderer::new(context, secrets);
        let url = renderer.url(&variant.request.url)?;
        let mut url = Url::parse(&url).map_err(|error| invalid(format!("request URL: {error}")))?;
        let mut pairs = Vec::new();
        for (name, value) in &variant.request.query {
            if let Some(value) = renderer.optional(value)? {
                pairs.push((name.as_str(), value));
            }
        }
        if !pairs.is_empty() {
            url.query_pairs_mut().extend_pairs(pairs);
        }
        if origin_of(&url) != variant.origin {
            return Err(invalid("the request URL left the spec's origin".to_owned()));
        }
        let mut headers = Vec::with_capacity(variant.request.headers.len() + 1);
        for (name, value) in &variant.request.headers {
            headers.push((name.clone(), renderer.header(value)?));
        }
        let body = match &variant.request.body {
            None => None,
            Some(Value::String(text)) => Some(renderer.text(text)?.into_bytes()),
            Some(tree) => {
                if !headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                {
                    headers.push(("Content-Type".to_owned(), "application/json".to_owned()));
                }
                Some(renderer.json(tree)?.to_string().into_bytes())
            }
        };
        Ok(RenderedRequest {
            method: variant.request.method,
            url,
            headers,
            body,
            secret_values: renderer.used_secrets().to_vec(),
        })
    }

    /// Reads the response of page `page` of an `engine` query.
    ///
    /// # Errors
    /// As [`CompiledMapping::parse`]; [`SerpError::InvalidSpec`] for an unsupported engine.
    pub fn parse(
        &self,
        engine: Engine,
        body: &str,
        url: &Url,
        page: u16,
    ) -> Result<ParsedPage, SerpError> {
        self.mapping(engine)
            .ok_or_else(|| invalid(format!("`{}` does not search {engine}", self.spec.id)))?
            .parse(body, url, page)
    }

    /// What the status code inside a JSON response means.
    pub fn status_of(&self, body: &Value) -> StatusOutcome {
        let (Some(probes), Some(rules)) = (&self.status, &self.spec.status) else {
            return StatusOutcome::Ok;
        };
        let Some(code) = probes
            .iter()
            .find_map(|probe| probe.json(body).into_iter().find_map(json_text))
        else {
            return StatusOutcome::Ok;
        };
        let message = self
            .status_message
            .iter()
            .find_map(|probe| probe.json(body).into_iter().find_map(json_text));
        let code = match message {
            Some(message) => format!("{code}: {message}"),
            None => code,
        };
        let bare = code.split(':').next().unwrap_or_default().to_owned();
        let listed = |codes: &[String]| codes.contains(&bare);
        if listed(&rules.ok)
            || (rules.ok.is_empty()
                && !listed(&rules.auth)
                && !listed(&rules.quota)
                && !listed(&rules.rate_limited))
        {
            StatusOutcome::Ok
        } else if listed(&rules.auth) {
            StatusOutcome::Auth(code)
        } else if listed(&rules.quota) {
            StatusOutcome::Quota(code)
        } else if listed(&rules.rate_limited) {
            StatusOutcome::RateLimited(code)
        } else {
            StatusOutcome::Error(code)
        }
    }

    /// What a JSON response reports it cost.
    pub fn usage_of(&self, body: &Value) -> ReportedUsage {
        let number = |probes: &[Probe]| {
            probes.iter().find_map(|probe| {
                probe.json(body).into_iter().find_map(|value| {
                    value
                        .as_f64()
                        .or_else(|| value.as_str()?.trim().parse().ok())
                })
            })
        };
        let credits = number(&self.usage_credits);
        let cost_usd = number(&self.usage_cost).or_else(|| {
            let per_credit = self.spec.cost.and_then(|cost| cost.per_credit_usd)?;
            credits.map(|credits| credits * per_credit)
        });
        ReportedUsage { cost_usd, credits }
    }
}

fn invalid(message: String) -> SerpError {
    SerpError::InvalidSpec(message)
}

/// Whether `id` is a valid provider id.
pub fn is_provider_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .bytes()
            .next()
            .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_header_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

/// The origin of a URL template, which must be written without placeholders.
fn literal_origin(template: &str) -> Result<String, SerpError> {
    let after_scheme = template
        .find("://")
        .map(|index| index + 3)
        .ok_or_else(|| invalid("the URL must start with http:// or https://".to_owned()))?;
    let authority_end = template[after_scheme..]
        .find(['/', '?', '#'])
        .map_or(template.len(), |index| after_scheme + index);
    let origin_part = &template[..authority_end];
    if origin_part.contains('{') || origin_part.contains('}') {
        return Err(invalid(
            "the scheme, host and port of the URL may not contain placeholders".to_owned(),
        ));
    }
    let url = Url::parse(origin_part).map_err(|error| invalid(format!("request URL: {error}")))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(invalid(
            "the URL must be http or https with a host".to_owned(),
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid(
            "credentials in the URL are not allowed; use a secret in a header".to_owned(),
        ));
    }
    Ok(origin_of(&url))
}

fn origin_of(url: &Url) -> String {
    url.origin().ascii_serialization()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn acme() -> ProviderSpec {
        serde_json::from_value(json!({
            "version": 1, "id": "acme", "name": "Acme SERP", "engines": ["google"],
            "secrets": [{"name": "token"}],
            "request": {"method": "POST", "url": "https://api.acme.example/v1/search?page={page}",
                "headers": {"Authorization": "Bearer {secret:token}"},
                "body": {"q": "{keyword}", "gl": "{gl}", "num": "{num:int}"}},
            "paging": {"per_page": 10, "first_page": 1, "max_pages": 2},
            "response": {"format": "json", "error": "$.error.message", "total": "$.info.total",
                "results": {"items": "$.organic[*]",
                    "fields": {"url": "$.link", "title": "$.title", "snippet": "$.snippet", "position": "$.position"}},
                "features": [{"kind": "paa", "items": "$.related_questions[*]", "fields": {"text": "$.question"}}]},
            "cost": {"per_request_usd": 0.002}, "rate_limit": {"per_minute": 60}
        }))
        .unwrap()
    }

    #[test]
    fn renders_requests_and_parses_responses() {
        let spec = acme().compile().unwrap();
        assert_eq!(
            spec.origin(Engine::Google),
            Some("https://api.acme.example")
        );
        assert_eq!(spec.origins(), ["https://api.acme.example"]);
        let query = SerpQuery::new(Engine::Google, "seo").with_depth(30);
        assert_eq!(spec.pages(&query), 2);
        assert!(spec.supports(&query));
        assert!(!spec.supports(&SerpQuery::new(Engine::Yandex, "seo")));

        let secrets = |name: &str| (name == "token").then(|| "abcd1234".to_owned());
        let request = spec.render(&query, 2, &secrets).unwrap();
        assert_eq!(
            request.url.as_str(),
            "https://api.acme.example/v1/search?page=2"
        );
        assert!(
            request
                .headers
                .contains(&("Authorization".to_owned(), "Bearer abcd1234".to_owned()))
        );
        assert!(
            request
                .headers
                .iter()
                .any(|(name, _)| name == "Content-Type")
        );
        let body: Value = serde_json::from_slice(request.body.as_deref().unwrap()).unwrap();
        assert_eq!(body, json!({"q": "seo", "gl": "tr", "num": 10}));
        assert_eq!(request.redact("key=abcd1234"), "key=***");

        let response = json!({
            "info": {"total": "About 2,000 results"},
            "organic": [
                {"position": 2, "link": "https://b.example/", "title": "B"},
                {"position": 1, "link": "https://a.example/x", "title": "A", "snippet": "a  b"},
                {"position": 3, "link": "ftp://c.example/", "title": "C"}
            ],
            "related_questions": [{"question": "Ne?"}, {"question": "Nasıl?"}]
        });
        let page = spec
            .parse(Engine::Google, &response.to_string(), &request.url, 2)
            .unwrap();
        assert_eq!(page.total_results, Some(2000));
        assert_eq!(page.organic.len(), 2);
        assert_eq!(page.organic[0].url, "https://a.example/x");
        assert_eq!(page.organic[0].snippet.as_deref(), Some("a b"));
        assert_eq!(page.organic[1].position, 2);
        assert_eq!(page.coverage.matched, 3);
        assert_eq!(page.features[0].items.len(), 2);
        assert!(spec.render(&query, 1, &|_| None).is_err());
    }

    #[test]
    fn rejects_unsafe_or_inconsistent_specs() {
        type Change = Box<dyn Fn(&mut ProviderSpec)>;
        let cases: Vec<(&str, Change)> = vec![
            ("id", Box::new(|spec| spec.id = "Acme!".to_owned())),
            ("engines", Box::new(|spec| spec.engines.clear())),
            (
                "duplicate engine",
                Box::new(|spec| spec.engines.push(Engine::Google)),
            ),
            (
                "host placeholder",
                Box::new(|spec| spec.request.url = "https://{hl}.acme.example/s".to_owned()),
            ),
            ("undeclared secret", Box::new(|spec| spec.secrets.clear())),
            (
                "credentials",
                Box::new(|spec| spec.request.url = "https://u:p@acme.example/s".to_owned()),
            ),
            (
                "scheme",
                Box::new(|spec| spec.request.url = "ftp://acme.example/s".to_owned()),
            ),
            (
                "header",
                Box::new(|spec| {
                    spec.request
                        .headers
                        .insert("Bad Header".to_owned(), "x".to_owned());
                }),
            ),
            (
                "get body",
                Box::new(|spec| spec.request.method = Method::Get),
            ),
            ("per page", Box::new(|spec| spec.paging.per_page = 0)),
            (
                "cost",
                Box::new(|spec| {
                    spec.cost = Some(CostSpec {
                        per_request_usd: -1.0,
                        per_credit_usd: None,
                    });
                }),
            ),
            ("version", Box::new(|spec| spec.version = 2)),
            (
                "override of an unlisted engine",
                Box::new(|spec| {
                    spec.engine_overrides
                        .insert(Engine::Bing, EngineOverride::default());
                }),
            ),
        ];
        for (case, change) in cases {
            let mut spec = acme();
            change(&mut spec);
            assert!(spec.compile().is_err(), "{case} should be rejected");
        }
    }

    #[test]
    fn regex_providers_read_named_groups() {
        let spec: ProviderSpec = serde_json::from_value(json!({
            "id": "local-yx", "name": "Local", "engines": ["yandex"],
            "request": {"url": "http://127.0.0.1:8080/serp?text={keyword}&lr={lr}&p={page0}"},
            "response": {"format": "text", "results": {
                "regex": "<li data-pos=\"(?P<position>\\d+)\"><a href=\"(?P<url>[^\"]+)\">(?P<title>[^<]+)</a><p>(?P<snippet>[^<]*)</p>",
                "decode_entities": true}}
        }))
        .unwrap();
        let spec = spec.compile().unwrap();
        let query = SerpQuery::new(Engine::Yandex, "kahve");
        let request = spec.render(&query, 1, &|_| None).unwrap();
        assert_eq!(
            request.url.as_str(),
            "http://127.0.0.1:8080/serp?text=kahve&lr=983&p=0"
        );
        let body = "<li data-pos=\"1\"><a href=\"https://x.example/\">X &amp; Y</a><p>s</p></li>";
        let page = spec.parse(Engine::Yandex, body, &request.url, 1).unwrap();
        assert_eq!(page.organic[0].title, "X & Y");
    }

    #[test]
    fn overrides_engines_and_reads_status_codes_and_costs() {
        let spec: ProviderSpec = serde_json::from_value(json!({
            "id": "multi", "name": "Multi", "engines": ["google", "yandex"],
            "secrets": [{"name": "login"}],
            "request": {"method": "POST", "url": "https://api.multi.example/v3/{engine}/live",
                "headers": {"Authorization": "Basic {secret:login:base64}"},
                "body": [{"keyword": "{keyword}", "location_name": "{google_location}", "depth": "{depth:int}"}]},
            "paging": {"per_page": 100, "max_pages": 1},
            "response": {"format": "json",
                "results": {"items": "$.tasks[0].result[0].items[?@.type == 'organic']",
                    "fields": {"url": "$.url", "title": "$.title", "position": "$.rank_group"}}},
            "engine_overrides": {"yandex": {"request": {"body": [{"keyword": "{keyword}", "lr": "{lr}"}]}}},
            "status": {"path": ["$.tasks[0].status_code", "$.status_code"], "message": "$.tasks[0].status_message",
                "ok": ["20000"], "auth": ["40100"], "quota": ["40210"]},
            "usage": {"cost_usd": "$.cost"},
            "cost": {"per_request_usd": 0.002}
        }))
        .unwrap();
        let spec = spec.compile().unwrap();
        let secrets = |_: &str| Some("me:pw".to_owned());
        let google = SerpQuery::new(Engine::Google, "kahve").with_depth(20);
        assert_eq!(spec.pages(&google), 1);
        assert!((spec.estimate_usd(&google) - 0.002).abs() < f64::EPSILON);
        let request = spec.render(&google, 1, &secrets).unwrap();
        assert_eq!(request.url.path(), "/v3/google/live");
        assert!(
            request
                .headers
                .contains(&("Authorization".to_owned(), "Basic bWU6cHc=".to_owned()))
        );
        assert_eq!(request.redact("Basic bWU6cHc="), "Basic ***");
        let body: Value = serde_json::from_slice(request.body.as_deref().unwrap()).unwrap();
        assert_eq!(
            body,
            json!([{"keyword": "kahve", "location_name": "Turkey", "depth": 20}])
        );

        let yandex = SerpQuery::new(Engine::Yandex, "kahve");
        let request = spec.render(&yandex, 1, &secrets).unwrap();
        assert_eq!(request.url.path(), "/v3/yandex/live");
        let body: Value = serde_json::from_slice(request.body.as_deref().unwrap()).unwrap();
        assert_eq!(body, json!([{"keyword": "kahve", "lr": "983"}]));

        let ok = json!({"cost": 0.0025, "tasks": [{"status_code": 20000, "result": [{"items": [
            {"type": "paid", "url": "https://ad.example/", "title": "Ad"},
            {"type": "organic", "rank_group": 1, "url": "https://a.example/", "title": "A"}
        ]}]}]});
        assert_eq!(spec.status_of(&ok), StatusOutcome::Ok);
        assert_eq!(spec.usage_of(&ok).cost_usd, Some(0.0025));
        let page = spec
            .parse(Engine::Google, &ok.to_string(), &request.url, 1)
            .unwrap();
        assert_eq!(page.organic.len(), 1);
        let unpaid =
            json!({"tasks": [{"status_code": 40210, "status_message": "Insufficient funds."}]});
        assert_eq!(
            spec.status_of(&unpaid),
            StatusOutcome::Quota("40210: Insufficient funds.".to_owned())
        );
        let unauthorised = json!({"status_code": 40100, "tasks": []});
        assert_eq!(
            spec.status_of(&unauthorised),
            StatusOutcome::Auth("40100".to_owned())
        );
        let odd = json!({"tasks": [{"status_code": 40501}]});
        assert_eq!(
            spec.status_of(&odd),
            StatusOutcome::Error("40501".to_owned())
        );
    }
}
