//! Paid sources: which keys they need, how each is paid now, budgets and spending.

use octopuson_cloud::{SourceMode, Via, connector_for};
use octopuson_keyword_sources::wordstat::Wordstat;
use octopuson_secrets::SecretState;
use serde::{Deserialize, Serialize};

use crate::{AppError, Workspace, month};

/// A paid source the platform knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderDef {
    /// Source id, as the gateway knows it (`connector_for`).
    pub id: &'static str,
    /// Owner of its keys in the credential store (Wordstat's are the Yandex Cloud account's).
    pub secrets_of: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Addresses its keys are sent to.
    pub origins: &'static [&'static str],
    /// Names of its keys.
    pub secrets: &'static [&'static str],
    /// Modules using it.
    pub modules: &'static [&'static str],
    /// Where an account and a key are made.
    pub signup_url: &'static str,
}

/// Name of the key of an AI assistant asked directly.
pub const LLM_KEY: &str = "api_key";

/// Every paid source.
pub const PROVIDERS: &[ProviderDef] = &[
    ProviderDef {
        id: octopuson_dataforseo::ID,
        secrets_of: octopuson_dataforseo::ID,
        name: "DataForSEO",
        origins: &[octopuson_dataforseo::ORIGIN],
        secrets: &[octopuson_dataforseo::SECRET],
        modules: &["overview", "keywords", "backlinks", "gap", "rank", "ai"],
        signup_url: "https://app.dataforseo.com/register",
    },
    ProviderDef {
        id: "openai",
        secrets_of: "openai",
        name: "OpenAI (ChatGPT)",
        origins: &["https://api.openai.com"],
        secrets: &[LLM_KEY],
        modules: &["ai"],
        signup_url: "https://platform.openai.com/api-keys",
    },
    ProviderDef {
        id: "anthropic",
        secrets_of: "anthropic",
        name: "Anthropic (Claude)",
        origins: &["https://api.anthropic.com"],
        secrets: &[LLM_KEY],
        modules: &["ai"],
        signup_url: "https://console.anthropic.com/settings/keys",
    },
    ProviderDef {
        id: "google-gemini",
        secrets_of: "google-gemini",
        name: "Google Gemini",
        origins: &["https://generativelanguage.googleapis.com"],
        secrets: &[LLM_KEY],
        modules: &["ai"],
        signup_url: "https://aistudio.google.com/apikey",
    },
    ProviderDef {
        id: "perplexity",
        secrets_of: "perplexity",
        name: "Perplexity",
        origins: &["https://api.perplexity.ai"],
        secrets: &[LLM_KEY],
        modules: &["ai"],
        signup_url: "https://www.perplexity.ai/account/api",
    },
    ProviderDef {
        id: Wordstat::ID,
        secrets_of: Wordstat::SECRETS,
        name: "Yandex Wordstat",
        origins: &[Wordstat::ORIGIN],
        secrets: &["api_key", "folder_id"],
        modules: &["keywords"],
        signup_url: "https://console.yandex.cloud/",
    },
];

/// A source by id.
///
/// # Errors
/// [`AppError::NotFound`] for an unknown id.
pub fn provider(id: &str) -> Result<&'static ProviderDef, AppError> {
    PROVIDERS
        .iter()
        .find(|definition| definition.id == id)
        .ok_or_else(|| AppError::NotFound(format!("source `{id}`")))
}

/// A key of a source, without its value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyInfo {
    /// Key name.
    pub name: String,
    /// `unset`, `saved`, `environment` or `rebind`.
    pub state: SecretState,
}

/// A source as the Sources page shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Source id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Modules using it.
    pub modules: Vec<String>,
    /// Its keys.
    pub keys: Vec<KeyInfo>,
    /// What the user chose.
    pub mode: SourceMode,
    /// How it is paid now; `None` when it cannot be used.
    pub via: Option<Via>,
    /// Every own key is available.
    pub own_ready: bool,
    /// The gateway offers it on credits (gateway connector id).
    pub credit_connector: Option<String>,
    /// Credits per US dollar of provider price, when on offer.
    pub credits_per_usd: Option<f64>,
    /// Most US dollars a month.
    pub budget_usd: Option<f64>,
    /// US dollars spent this month.
    pub spent_usd: f64,
    /// Where an account and a key are made.
    pub signup_url: String,
}

impl Workspace {
    /// Every paid source with its keys and how it is paid now.
    ///
    /// # Errors
    /// [`AppError`] when the settings, the credential store or the database cannot be read.
    pub fn providers(&self) -> Result<Vec<ProviderInfo>, AppError> {
        let settings = self.settings()?;
        self.warm_catalog(&settings)?;
        let store = self.store()?;
        let month = month();
        let mut providers = Vec::with_capacity(PROVIDERS.len());
        for definition in PROVIDERS {
            let keys = definition
                .secrets
                .iter()
                .map(|name| {
                    Ok(KeyInfo {
                        name: (*name).to_owned(),
                        state: self.secrets.state(
                            definition.secrets_of,
                            name,
                            definition.origins,
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, AppError>>()?;
            let credit = self.credit_connector(definition.id);
            providers.push(ProviderInfo {
                id: definition.id.to_owned(),
                name: definition.name.to_owned(),
                modules: definition.modules.iter().map(|&m| m.to_owned()).collect(),
                keys,
                mode: settings.mode(definition.id),
                via: self.via(&settings, definition.id),
                own_ready: self.own_ready(definition.id),
                credit_connector: credit.map(|connector| connector.id),
                credits_per_usd: self.credits_per_usd(definition.id),
                budget_usd: settings.budgets.get(definition.id).copied(),
                spent_usd: store.spent_in(definition.id, &month)?,
                signup_url: definition.signup_url.to_owned(),
            });
        }
        Ok(providers)
    }

    /// Saves an own key of a source, bound to its addresses.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for an unknown source or key, [`AppError::Invalid`] for an
    /// empty value or DataForSEO credentials that are not `login:password`.
    pub fn set_key(&self, provider_id: &str, name: &str, value: &str) -> Result<(), AppError> {
        let definition = provider(provider_id)?;
        if !definition.secrets.contains(&name) {
            return Err(AppError::NotFound(format!(
                "key `{name}` of `{provider_id}`"
            )));
        }
        let value = value.trim();
        if value.is_empty() {
            return Err(AppError::Invalid("the key is empty".to_owned()));
        }
        if provider_id == octopuson_dataforseo::ID
            && !value
                .split_once(':')
                .is_some_and(|(login, password)| !login.is_empty() && !password.is_empty())
        {
            return Err(AppError::Invalid(
                "DataForSEO: enter login:password (API login and API password)".to_owned(),
            ));
        }
        self.secrets
            .set(definition.secrets_of, name, value, definition.origins)?;
        Ok(())
    }

    /// Removes an own key.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for an unknown source, or credential store errors.
    pub fn remove_key(&self, provider_id: &str, name: &str) -> Result<(), AppError> {
        let definition = provider(provider_id)?;
        self.secrets.delete(definition.secrets_of, name)?;
        Ok(())
    }

    /// Chooses how a source is paid.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for an unknown source, [`AppError::Invalid`] for credits on a
    /// source the gateway has no connector for.
    pub fn set_source_mode(&self, provider_id: &str, mode: SourceMode) -> Result<(), AppError> {
        provider(provider_id)?;
        if mode == SourceMode::Credit && connector_for(provider_id).is_none() {
            return Err(AppError::Invalid(format!(
                "`{provider_id}` cannot run on credits"
            )));
        }
        self.update_settings(|settings| {
            settings.modes.insert(provider_id.to_owned(), mode);
        })?;
        Ok(())
    }

    /// Sets (or with `None` removes) a source's monthly budget in US dollars.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for an unknown source, [`AppError::Invalid`] for a negative amount.
    pub fn set_budget(&self, provider_id: &str, usd: Option<f64>) -> Result<(), AppError> {
        provider(provider_id)?;
        self.update_settings(|settings| match usd {
            Some(usd) => {
                settings.budgets.insert(provider_id.to_owned(), usd);
            }
            None => {
                settings.budgets.remove(provider_id);
            }
        })?;
        Ok(())
    }
}
