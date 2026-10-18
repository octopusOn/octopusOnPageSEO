//! The paid keyword sources: their keys, monthly budgets and whether they can be used.

use seonexus_keyword_sources::SourceContext;
use seonexus_keyword_sources::dataforseo::DataForSeo;
use seonexus_keyword_sources::serpapi::SerpApi;
use seonexus_keyword_sources::suggest::FreeSuggest;
use seonexus_keyword_sources::wordstat::Wordstat;
use seonexus_secrets::SecretState;
use serde::{Deserialize, Serialize};

use crate::settings::Settings;
use crate::{KeywordWorkspace, KwAppError};

/// User agent of the SEO Keyword Research's requests.
pub const USER_AGENT: &str = concat!("octopusSEO-Keyword-Miner/", env!("CARGO_PKG_VERSION"));

/// A paid source as the app knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderDef {
    /// Provider id (budgets, usage, switches).
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Owner of its secrets.
    pub secrets_of: &'static str,
    /// Secret names it needs.
    pub secrets: &'static [&'static str],
    /// Secret names it can use.
    pub optional: &'static [&'static str],
    /// Address the secrets are bound to.
    pub origin: &'static str,
    /// What it offers, for the interface.
    pub offers: &'static [&'static str],
    /// Where to get a key.
    pub signup: &'static str,
}

/// Owner of the Google secrets.
pub const GOOGLE: &str = "google";

/// Every source the SEO Keyword Research asks: paid keyword and result-page APIs, the user's Google
/// data, the attended browser mode and the free suggestion endpoints.
pub const PROVIDERS: &[ProviderDef] = &[
    ProviderDef {
        id: Wordstat::ID,
        name: "Yandex Wordstat",
        secrets_of: Wordstat::SECRETS,
        secrets: &["api_key", "folder_id"],
        optional: &[],
        origin: Wordstat::ORIGIN,
        offers: &["discover", "volume", "trend"],
        signup: "https://yandex.cloud/en/docs/search-api/",
    },
    ProviderDef {
        id: DataForSeo::ID,
        name: "DataForSEO",
        secrets_of: DataForSeo::ID,
        secrets: &["credentials"],
        optional: &[],
        origin: DataForSeo::ORIGIN,
        offers: &[
            "discover",
            "volume",
            "difficulty",
            "intent",
            "competitors",
            "autocomplete",
        ],
        signup: "https://app.dataforseo.com/register",
    },
    ProviderDef {
        id: SerpApi::ID,
        name: "SerpApi",
        secrets_of: SerpApi::ID,
        secrets: &["api_key"],
        optional: &[],
        origin: SerpApi::ORIGIN,
        offers: &["autocomplete"],
        signup: "https://serpapi.com/users/sign_up",
    },
    ProviderDef {
        id: seonexus_keyword_sources::search_console::SearchConsole::ID,
        name: "Google Search Console",
        secrets_of: GOOGLE,
        secrets: &["client_secret", "refresh_token"],
        optional: &[],
        origin: seonexus_oauth::TOKEN_ORIGIN,
        offers: &["discover", "site pages"],
        signup: "https://console.cloud.google.com/apis/credentials",
    },
    ProviderDef {
        id: seonexus_keyword_sources::google_ads::GoogleAds::ID,
        name: "Google Ads Keyword Planner",
        secrets_of: GOOGLE,
        secrets: &["client_secret", "refresh_token"],
        optional: &["developer_token"],
        origin: seonexus_oauth::TOKEN_ORIGIN,
        offers: &["discover", "volume"],
        signup: "https://developers.google.com/google-ads/api/docs/get-started/introduction",
    },
    ProviderDef {
        id: "yandex-cloud",
        name: "Yandex Search API",
        secrets_of: Wordstat::SECRETS,
        secrets: &["api_key", "folder_id"],
        optional: &[],
        origin: Wordstat::ORIGIN,
        offers: &["result pages"],
        signup: "https://yandex.cloud/en/docs/search-api/",
    },
    ProviderDef {
        id: "serper",
        name: "Serper",
        secrets_of: "serper",
        secrets: &["api_key"],
        optional: &[],
        origin: "https://google.serper.dev",
        offers: &["result pages"],
        signup: "https://serper.dev",
    },
    ProviderDef {
        id: seonexus_serp::provider::BROWSER_ID,
        name: "Browser (attended)",
        secrets_of: seonexus_serp::provider::BROWSER_ID,
        secrets: &[],
        optional: &[],
        origin: "",
        offers: &["result pages"],
        signup: "",
    },
    ProviderDef {
        id: FreeSuggest::GOOGLE,
        name: "Google suggestions (free, unofficial)",
        secrets_of: FreeSuggest::GOOGLE,
        secrets: &[],
        optional: &[],
        origin: "https://suggestqueries.google.com",
        offers: &["autocomplete"],
        signup: "",
    },
    ProviderDef {
        id: FreeSuggest::YANDEX,
        name: "Yandex suggestions (free, unofficial)",
        secrets_of: FreeSuggest::YANDEX,
        secrets: &[],
        optional: &[],
        origin: "https://suggest.yandex.com",
        offers: &["autocomplete"],
        signup: "",
    },
];

/// The definition of a provider.
///
/// # Errors
/// [`KwAppError::Invalid`] for an unknown id.
pub fn provider(id: &str) -> Result<&'static ProviderDef, KwAppError> {
    PROVIDERS
        .iter()
        .find(|provider| provider.id == id)
        .or_else(|| PROVIDERS.iter().find(|provider| provider.secrets_of == id))
        .ok_or_else(|| {
            KwAppError::Invalid(format!(
                "unknown provider `{id}` ({})",
                PROVIDERS
                    .iter()
                    .map(|provider| provider.id)
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })
}

/// A secret of a provider and where it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretInfo {
    /// Secret name.
    pub name: String,
    /// Saved, from the environment, unset, or saved for another address.
    pub state: SecretState,
}

/// A provider with its state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Provider id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Its secrets.
    pub secrets: Vec<SecretInfo>,
    /// Usable now: every own key is available, or it runs on credits.
    pub ready: bool,
    /// How it is paid now, when usable.
    pub via: Option<seonexus_cloud::Via>,
    /// What the user chose.
    pub mode: seonexus_cloud::SourceMode,
    /// The OctopusOn SEO credit offer, when signed in and on offer.
    pub credit: Option<crate::cloud::CreditOffer>,
    /// Not switched off in the settings.
    pub enabled: bool,
    /// Monthly budget, US dollars.
    pub budget_usd: Option<f64>,
    /// Spent this month, US dollars.
    pub spent_usd: f64,
    /// What it offers.
    pub offers: Vec<String>,
    /// Where to get a key.
    pub signup: String,
}

impl KeywordWorkspace {
    /// The user's own secret of a provider, if it is available for the provider's address.
    pub(crate) fn own_secret(&self, owner: &str, name: &str) -> Option<String> {
        let origin = PROVIDERS
            .iter()
            .find(|provider| provider.secrets_of == owner)?
            .origin;
        self.secrets().get(owner, name, &[origin]).ok().flatten()
    }

    /// Runs `work` with a source context over `transport`.
    pub(crate) fn with_sources<T>(
        &self,
        transport: &dyn seonexus_core::http::HttpTransport,
        work: impl FnOnce(&SourceContext<'_>) -> T,
    ) -> T {
        let lookup = |owner: &str, name: &str| self.secret(owner, name);
        work(&SourceContext {
            transport,
            secrets: &lookup,
        })
    }

    /// The transport of the user's network settings, without credit routing.
    ///
    /// # Errors
    /// [`KwAppError::InvalidSettings`] for an unusable proxy.
    pub fn plain_transport(
        &self,
        settings: &Settings,
    ) -> Result<seonexus_net::ApiClient, KwAppError> {
        seonexus_net::ApiClient::new(
            USER_AGENT,
            std::time::Duration::from_secs(settings.network.timeout_secs),
            settings.network.proxy.as_deref(),
        )
        .map_err(|error| KwAppError::InvalidSettings(error.to_string()))
    }

    /// The transport of the user's network settings: requests of sources on OctopusOn SEO
    /// credits go through the gateway, everything else directly.
    ///
    /// # Errors
    /// [`KwAppError::InvalidSettings`] for an unusable proxy.
    pub fn transport(
        &self,
        settings: &Settings,
    ) -> Result<seonexus_cloud::CreditTransport<seonexus_net::ApiClient>, KwAppError> {
        let inner = self.plain_transport(settings)?;
        self.refresh_catalog(&inner);
        let connectors = self.credit_connectors(settings);
        Ok(seonexus_cloud::CreditTransport::new(
            inner,
            self.cloud_api(),
            self.cloud_token().unwrap_or_default(),
            crate::cloud::app_name(),
            connectors,
        ))
    }

    /// Every paid source with its keys, switch, budget and this month's spending.
    ///
    /// # Errors
    /// Settings, secret store and store errors.
    pub fn providers(&self) -> Result<Vec<ProviderInfo>, KwAppError> {
        let settings = self.settings()?;
        if self.cloud_token().is_some() {
            self.refresh_catalog(&self.plain_transport(&settings)?);
        }
        let store = self.store()?;
        let month = crate::month();
        PROVIDERS
            .iter()
            .map(|provider| {
                let secrets = provider
                    .secrets
                    .iter()
                    .map(|name| {
                        let state = self
                            .secrets()
                            .state(provider.secrets_of, name, &[provider.origin])
                            .map_err(|error| KwAppError::Secrets(error.to_string()))?;
                        Ok(SecretInfo {
                            name: (*name).to_owned(),
                            state,
                        })
                    })
                    .collect::<Result<Vec<_>, KwAppError>>()?;
                let via = self.via(&settings, provider.id);
                Ok(ProviderInfo {
                    id: provider.id.to_owned(),
                    name: provider.name.to_owned(),
                    ready: via.is_some()
                        || secrets.iter().all(|secret| {
                            matches!(secret.state, SecretState::Saved | SecretState::Environment)
                        }),
                    via,
                    mode: settings
                        .credit
                        .get(provider.id)
                        .copied()
                        .unwrap_or_default(),
                    credit: self.credit_offer(provider.id),
                    secrets,
                    enabled: !settings.disabled.contains(provider.id),
                    budget_usd: settings.budgets.get(provider.id).copied(),
                    spent_usd: store.spent(provider.id, &month)?,
                    offers: provider
                        .offers
                        .iter()
                        .map(|offer| (*offer).to_owned())
                        .collect(),
                    signup: provider.signup.to_owned(),
                })
            })
            .collect()
    }

    /// Saves a key of a provider in the system keychain, bound to the provider's address.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for an unknown provider or secret; secret store errors.
    pub fn set_key(&self, provider_id: &str, name: &str, value: &str) -> Result<(), KwAppError> {
        let provider = provider(provider_id)?;
        if name == "refresh_token" {
            return Err(KwAppError::Invalid(
                "the refresh token comes from signing in (`octopuson-keywords google login`)"
                    .to_owned(),
            ));
        }
        if !provider.secrets.contains(&name) && !provider.optional.contains(&name) {
            return Err(KwAppError::Invalid(format!(
                "{} has no key `{name}` ({})",
                provider.name,
                provider
                    .secrets
                    .iter()
                    .chain(provider.optional)
                    .filter(|name| **name != "refresh_token")
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        if provider.id == DataForSeo::ID && !value.contains(':') {
            return Err(KwAppError::Invalid(
                "DataForSEO credentials are login:password".to_owned(),
            ));
        }
        self.secrets()
            .set(provider.secrets_of, name, value, &[provider.origin])
            .map_err(|error| KwAppError::Secrets(error.to_string()))
    }

    /// Removes a key of a provider.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for an unknown provider; secret store errors.
    pub fn remove_key(&self, provider_id: &str, name: &str) -> Result<(), KwAppError> {
        let provider = provider(provider_id)?;
        self.secrets()
            .delete(provider.secrets_of, name)
            .map_err(|error| KwAppError::Secrets(error.to_string()))
    }

    /// Sets (or with `None` removes) a provider's monthly budget.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for an unknown provider; settings errors.
    pub fn set_budget(&self, provider_id: &str, budget_usd: Option<f64>) -> Result<(), KwAppError> {
        let provider = provider(provider_id)?;
        let mut settings = self.settings()?;
        match budget_usd {
            Some(budget) => settings.budgets.insert(provider.id.to_owned(), budget),
            None => settings.budgets.remove(provider.id),
        };
        self.save_settings(&settings)
    }

    /// Switches a provider on or off.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for an unknown provider; settings errors.
    pub fn set_enabled(&self, provider_id: &str, enabled: bool) -> Result<(), KwAppError> {
        let provider = provider(provider_id)?;
        let mut settings = self.settings()?;
        if enabled {
            settings.disabled.remove(provider.id);
        } else {
            settings.disabled.insert(provider.id.to_owned());
        }
        self.save_settings(&settings)
    }
}
