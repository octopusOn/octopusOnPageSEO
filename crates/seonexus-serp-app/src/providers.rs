//! The providers a workspace can use: built-in APIs and the user's own specs.

use std::fs;
use std::path::Path;

use seonexus_serp::mapping::ParsedPage;
use seonexus_serp::provider::{SerpProvider, SpecProvider, YandexCloud, builtin_specs};
use seonexus_serp::spec::{ProviderSpec, SecretSpec};
use seonexus_serp::{Device, Engine};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::secrets::{SecretState, Secrets};
use crate::settings::Settings;
use crate::{SerpAppError, files::write_atomically};

/// Id of the attended browser provider.
pub const BROWSER: &str = "browser";

const PROVIDERS_FILE_VERSION: u32 = 1;

/// Where a provider comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    /// Shipped with the program.
    Builtin,
    /// Defined by the user.
    Custom,
    /// The user's own browser.
    Browser,
}

/// A secret a provider needs, and whether it is there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretInfo {
    /// Name used in templates.
    pub name: String,
    /// Label to ask for it with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Whether it is set.
    pub state: SecretState,
    /// Environment variable that can set it.
    pub variable: String,
}

/// A provider as shown to the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Origin.
    pub kind: ProviderKind,
    /// Engines.
    pub engines: Vec<Engine>,
    /// Devices.
    pub devices: Vec<Device>,
    /// Secrets.
    pub secrets: Vec<SecretInfo>,
    /// Origins requests go to (and secrets are bound to).
    pub origins: Vec<String>,
    /// Whether it can be used: every secret set and the spec valid, or on credits.
    pub ready: bool,
    /// How it is paid now, when usable.
    #[serde(default)]
    pub via: Option<seonexus_cloud::Via>,
    /// What the user chose.
    #[serde(default)]
    pub mode: seonexus_cloud::SourceMode,
    /// The OctopusOn SEO credit offer, when signed in and on offer.
    #[serde(default)]
    pub credit: Option<crate::cloud::CreditOffer>,
    /// Whether it is switched on.
    pub enabled: bool,
    /// Price per request, US dollars.
    pub per_request_usd: f64,
    /// Monthly budget, US dollars.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_usd: Option<f64>,
    /// Notes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Why the spec cannot be used, for an invalid custom spec.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A usable provider with what it needs.
pub(crate) struct Entry {
    pub(crate) provider: Box<dyn SerpProvider>,
    pub(crate) kind: ProviderKind,
    pub(crate) secrets: Vec<SecretSpec>,
    pub(crate) origins: Vec<String>,
    pub(crate) per_request_usd: f64,
    pub(crate) notes: Option<String>,
}

impl Entry {
    pub(crate) fn origin_refs(&self) -> Vec<&str> {
        self.origins.iter().map(String::as_str).collect()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProvidersFile {
    version: u32,
    providers: Vec<ProviderSpec>,
}

/// Ids the user may not take.
pub fn reserved_ids() -> Vec<String> {
    let mut ids: Vec<String> = builtin_specs()
        .unwrap_or_default()
        .into_iter()
        .map(|spec| spec.id)
        .collect();
    ids.push(YandexCloud::ID.to_owned());
    ids.push(BROWSER.to_owned());
    ids.push("wordstat".to_owned());
    ids.push("dataforseo-ads".to_owned());
    ids.push("open-pagerank".to_owned());
    ids
}

/// User specs in `providers.json`.
pub(crate) fn load_custom(path: &Path) -> Result<Vec<ProviderSpec>, SerpAppError> {
    match fs::read_to_string(path) {
        Ok(text) => parse_specs(&text).map_err(|error| SerpAppError::invalid_file(path, error)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(SerpAppError::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

pub(crate) fn save_custom(path: &Path, specs: &[ProviderSpec]) -> Result<(), SerpAppError> {
    let file = ProvidersFile {
        version: PROVIDERS_FILE_VERSION,
        providers: specs.to_vec(),
    };
    let text = serde_json::to_string_pretty(&file)
        .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?;
    write_atomically(path, text.as_bytes())
}

/// Reads specs from `{"providers": [...]}`, a bare array or a single spec.
///
/// # Errors
/// A message for invalid JSON.
pub fn parse_specs(text: &str) -> Result<Vec<ProviderSpec>, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let list = match value {
        serde_json::Value::Object(ref object) if object.contains_key("providers") => {
            object["providers"].clone()
        }
        serde_json::Value::Array(_) => value,
        single => serde_json::Value::Array(vec![single]),
    };
    serde_json::from_value(list).map_err(|error| error.to_string())
}

/// Every provider, usable or not: (entry, error of an invalid custom spec).
pub(crate) fn catalog(custom: Vec<ProviderSpec>) -> (Vec<Entry>, Vec<(ProviderSpec, String)>) {
    let mut entries = Vec::new();
    for spec in builtin_specs().unwrap_or_default() {
        if let Ok(compiled) = spec.compile() {
            entries.push(spec_entry(compiled, ProviderKind::Builtin));
        }
    }
    entries.push(Entry {
        provider: Box::new(YandexCloud),
        kind: ProviderKind::Builtin,
        secrets: vec![
            SecretSpec {
                name: "api_key".to_owned(),
                label: Some("API key (scope yc.search-api.execute)".to_owned()),
            },
            SecretSpec {
                name: "folder_id".to_owned(),
                label: Some("Folder ID".to_owned()),
            },
        ],
        origins: vec!["https://searchapi.api.cloud.yandex.net".to_owned()],
        per_request_usd: 0.004,
        notes: Some(
            "Yandex Search API (Yandex Cloud): yandex.com.tr and other Yandex sites; the same key gives Wordstat volumes."
                .to_owned(),
        ),
    });
    let reserved = reserved_ids();
    let mut invalid = Vec::new();
    for spec in custom {
        if reserved.contains(&spec.id) {
            let message = format!("`{}` is the id of a built-in provider", spec.id);
            invalid.push((spec, message));
            continue;
        }
        match spec.compile() {
            Ok(compiled) => entries.push(spec_entry(compiled, ProviderKind::Custom)),
            Err(error) => invalid.push((spec, error.to_string())),
        }
    }
    (entries, invalid)
}

fn spec_entry(compiled: seonexus_serp::spec::CompiledSpec, kind: ProviderKind) -> Entry {
    let spec = compiled.spec().clone();
    let origins = compiled.origins().into_iter().map(str::to_owned).collect();
    Entry {
        kind,
        secrets: spec.secrets.clone(),
        origins,
        per_request_usd: spec.cost.map_or(0.0, |cost| cost.per_request_usd),
        notes: spec.notes.clone(),
        provider: Box::new(SpecProvider::new(compiled)),
    }
}

/// What the user sees of an entry.
pub(crate) fn describe(
    entry: &Entry,
    secrets: &Secrets,
    settings: &Settings,
) -> Result<ProviderInfo, SerpAppError> {
    let provider = &entry.provider;
    let origins = entry.origin_refs();
    let secret_infos = entry
        .secrets
        .iter()
        .map(|secret| {
            Ok(SecretInfo {
                name: secret.name.clone(),
                label: secret.label.clone(),
                state: secrets.state(provider.id(), &secret.name, &origins)?,
                variable: Secrets::variable(provider.id(), &secret.name),
            })
        })
        .collect::<Result<Vec<_>, SerpAppError>>()?;
    let capabilities = provider.capabilities();
    Ok(ProviderInfo {
        id: provider.id().to_owned(),
        name: provider.name().to_owned(),
        kind: entry.kind,
        engines: capabilities.engines,
        devices: capabilities.devices,
        ready: secret_infos
            .iter()
            .all(|secret| matches!(secret.state, SecretState::Saved | SecretState::Environment)),
        secrets: secret_infos,
        origins: entry.origins.clone(),
        enabled: !settings.disabled.contains(provider.id()),
        per_request_usd: entry.per_request_usd,
        budget_usd: settings.budgets.get(provider.id()).copied(),
        notes: entry.notes.clone(),
        error: None,
        via: None,
        mode: seonexus_cloud::SourceMode::Auto,
        credit: None,
    })
}

/// Reads a response with a spec without sending anything, to test a mapping.
///
/// # Errors
/// [`SerpAppError::InvalidProvider`] for an invalid spec, an engine it does not search or a
/// response it cannot read.
pub fn test_spec(
    spec: &ProviderSpec,
    engine: Engine,
    body: &str,
    url: Option<&str>,
) -> Result<ParsedPage, SerpAppError> {
    let compiled = spec
        .compile()
        .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?;
    let url = match url {
        Some(url) => {
            Url::parse(url).map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?
        }
        None => Url::parse(compiled.origin(engine).ok_or_else(|| {
            SerpAppError::InvalidProvider(format!("`{}` does not search {engine}", spec.id))
        })?)
        .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?,
    };
    compiled
        .parse(engine, body, &url, 1)
        .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))
}
