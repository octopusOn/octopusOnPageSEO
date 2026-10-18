//! DataForSEO, with the `credentials` secret (`login:password`): Google Ads numbers and
//! ideas (Keywords Data), DataForSEO Labs (ideas, related keywords, suggestions, a domain's
//! ranking keywords, difficulty, intent) and Google autocomplete. Live endpoints only, so no
//! task polling is needed.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use seonexus_core::http::HttpRequest;
use seonexus_serp::locale::country_name;
use serde_json::{Value, json};

use crate::{
    Fetched, FoundKeyword, Market, Monthly, SourceContext, SourceError, count, parse, redact,
};

/// DataForSEO.
#[derive(Debug, Clone, Copy, Default)]
pub struct DataForSeo;

/// Prices used for estimates, US dollars (the response's own cost is recorded when given).
pub mod prices {
    /// A Google Ads (Keywords Data) live task.
    pub const ADS_TASK: f64 = 0.09;
    /// A Labs task.
    pub const LABS_TASK: f64 = 0.012;
    /// Each Labs result.
    pub const LABS_RESULT: f64 = 0.000_12;
    /// An autocomplete request.
    pub const AUTOCOMPLETE: f64 = 0.000_6;
}

impl DataForSeo {
    /// Source id.
    pub const ID: &'static str = "dataforseo";
    /// Address the secret is bound to.
    pub const ORIGIN: &'static str = "https://api.dataforseo.com";
    /// Keywords per volume, difficulty or intent task.
    pub const BATCH: usize = 1_000;
    /// Seeds per Google Ads ideas task.
    pub const ADS_SEEDS: usize = 20;
    /// Seeds per Labs ideas task.
    pub const LABS_SEEDS: usize = 200;
    /// Most results one Labs task returns.
    pub const LABS_LIMIT: u32 = 1_000;

    fn location(market: &Market) -> Result<&'static str, SourceError> {
        country_name(&market.country).ok_or_else(|| {
            SourceError::Config(format!(
                "DataForSEO: no location known for country `{}`",
                market.country
            ))
        })
    }

    /// Sends one task and returns its result list and cost.
    fn post(
        context: &SourceContext<'_>,
        path: &str,
        task: &Value,
    ) -> Result<(Vec<Value>, f64), SourceError> {
        let credentials = context.secret(Self::ID, "credentials")?;
        if !credentials.contains(':') {
            return Err(SourceError::Config(
                "the dataforseo key `credentials` must be login:password".to_owned(),
            ));
        }
        let authorization = format!("Basic {}", STANDARD.encode(&credentials));
        let hide = |text: &str| redact(text, &[credentials.clone(), authorization.clone()]);
        let request = HttpRequest::post(
            format!("{}/v3/{path}", Self::ORIGIN),
            json!([task]).to_string().into_bytes(),
        )
        .header("Authorization", authorization.clone())
        .header("Content-Type", "application/json");
        let response = context.send(&request)?;
        if !response.is_success() {
            return Err(SourceError::from_status(&response, &hide));
        }
        let value = parse(&response, "DataForSEO")?;
        let status = value
            .pointer("/tasks/0/status_code")
            .or_else(|| value.get("status_code"))
            .and_then(count);
        if let Some(status) = status.filter(|status| *status != 20_000) {
            let message = value
                .pointer("/tasks/0/status_message")
                .or_else(|| value.get("status_message"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let message = hide(&format!("{status}: {message}"));
            return Err(match status {
                40_100..=40_104 => SourceError::Auth(message),
                40_200 | 40_210 => SourceError::Quota {
                    message,
                    retry_after: None,
                },
                40_202 => SourceError::RateLimited { retry_after: None },
                _ => SourceError::Rejected(message),
            });
        }
        let cost = value
            .pointer("/tasks/0/cost")
            .or_else(|| value.get("cost"))
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let results = value
            .pointer("/tasks/0/result")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok((results, cost))
    }

    fn fetched(keywords: Vec<FoundKeyword>, cost: f64, estimate: f64) -> Fetched {
        Fetched {
            keywords,
            requests: 1,
            cost_usd: if cost > 0.0 { cost } else { estimate },
        }
    }

    /// Google Ads volumes, cost per click, competition and monthly searches of up to
    /// [`Self::BATCH`] keywords.
    ///
    /// # Errors
    /// [`SourceError`] when DataForSEO cannot be asked.
    pub fn search_volume(
        context: &SourceContext<'_>,
        keywords: &[String],
        market: &Market,
    ) -> Result<Fetched, SourceError> {
        let keywords = &keywords[..keywords.len().min(Self::BATCH)];
        let (results, cost) = Self::post(
            context,
            "keywords_data/google_ads/search_volume/live",
            &json!({
                "keywords": keywords,
                "location_name": Self::location(market)?,
                "language_code": market.language,
            }),
        )?;
        Ok(Self::fetched(
            results.iter().filter_map(ads_item).collect(),
            cost,
            prices::ADS_TASK,
        ))
    }

    /// Google Ads keyword ideas for up to [`Self::ADS_SEEDS`] seeds.
    ///
    /// # Errors
    /// [`SourceError`] when DataForSEO cannot be asked.
    pub fn keywords_for_keywords(
        context: &SourceContext<'_>,
        seeds: &[String],
        market: &Market,
    ) -> Result<Fetched, SourceError> {
        let seeds = &seeds[..seeds.len().min(Self::ADS_SEEDS)];
        let (results, cost) = Self::post(
            context,
            "keywords_data/google_ads/keywords_for_keywords/live",
            &json!({
                "keywords": seeds,
                "location_name": Self::location(market)?,
                "language_code": market.language,
            }),
        )?;
        Ok(Self::fetched(
            results.iter().filter_map(ads_item).collect(),
            cost,
            prices::ADS_TASK,
        ))
    }

    fn labs(
        context: &SourceContext<'_>,
        path: &str,
        mut task: Value,
        market: &Market,
        limit: u32,
    ) -> Result<Fetched, SourceError> {
        let limit = limit.clamp(1, Self::LABS_LIMIT);
        task["location_name"] = json!(Self::location(market)?);
        task["language_code"] = json!(market.language);
        task["limit"] = json!(limit);
        let (results, cost) = Self::post(
            context,
            &format!("dataforseo_labs/google/{path}/live"),
            &task,
        )?;
        let keywords: Vec<FoundKeyword> = results
            .iter()
            .flat_map(|result| {
                result
                    .get("items")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            })
            .filter_map(|item| labs_item(&item))
            .collect();
        let estimate = Self::labs_estimate(u32::try_from(keywords.len()).unwrap_or(limit));
        Ok(Self::fetched(keywords, cost, estimate))
    }

    /// Estimated price of a Labs task returning `results` results.
    pub fn labs_estimate(results: u32) -> f64 {
        prices::LABS_TASK + prices::LABS_RESULT * f64::from(results)
    }

    /// Labs keyword ideas (same topic, other wording) for up to [`Self::LABS_SEEDS`] seeds.
    ///
    /// # Errors
    /// [`SourceError`] when DataForSEO cannot be asked.
    pub fn keyword_ideas(
        context: &SourceContext<'_>,
        seeds: &[String],
        market: &Market,
        limit: u32,
    ) -> Result<Fetched, SourceError> {
        let seeds = &seeds[..seeds.len().min(Self::LABS_SEEDS)];
        Self::labs(
            context,
            "keyword_ideas",
            json!({ "keywords": seeds }),
            market,
            limit,
        )
    }

    /// Labs related keywords ("searches related to", `depth` 0–4 levels deep).
    ///
    /// # Errors
    /// [`SourceError`] when DataForSEO cannot be asked.
    pub fn related_keywords(
        context: &SourceContext<'_>,
        seed: &str,
        market: &Market,
        depth: u8,
        limit: u32,
    ) -> Result<Fetched, SourceError> {
        Self::labs(
            context,
            "related_keywords",
            json!({ "keyword": seed, "depth": depth.min(4) }),
            market,
            limit,
        )
    }

    /// Labs keyword suggestions (longer keywords containing the seed).
    ///
    /// # Errors
    /// [`SourceError`] when DataForSEO cannot be asked.
    pub fn keyword_suggestions(
        context: &SourceContext<'_>,
        seed: &str,
        market: &Market,
        limit: u32,
    ) -> Result<Fetched, SourceError> {
        Self::labs(
            context,
            "keyword_suggestions",
            json!({ "keyword": seed }),
            market,
            limit,
        )
    }

    /// Keywords a domain ranks for, with the ranking page and position.
    ///
    /// # Errors
    /// [`SourceError`] when DataForSEO cannot be asked.
    pub fn ranked_keywords(
        context: &SourceContext<'_>,
        domain: &str,
        market: &Market,
        limit: u32,
    ) -> Result<Fetched, SourceError> {
        Self::labs(
            context,
            "ranked_keywords",
            json!({
                "target": domain,
                "order_by": ["keyword_data.keyword_info.search_volume,desc"],
            }),
            market,
            limit,
        )
    }

    /// Keyword difficulty (0–100) of up to [`Self::BATCH`] keywords.
    ///
    /// # Errors
    /// [`SourceError`] when DataForSEO cannot be asked.
    pub fn keyword_difficulty(
        context: &SourceContext<'_>,
        keywords: &[String],
        market: &Market,
    ) -> Result<Fetched, SourceError> {
        let keywords = &keywords[..keywords.len().min(Self::BATCH)];
        let count = u32::try_from(keywords.len()).unwrap_or(u32::MAX);
        let mut fetched = Self::labs(
            context,
            "bulk_keyword_difficulty",
            json!({ "keywords": keywords }),
            market,
            count,
        )?;
        // The limit is not part of this task.
        fetched
            .keywords
            .retain(|keyword| keyword.difficulty.is_some());
        Ok(fetched)
    }

    /// Search intent of up to [`Self::BATCH`] keywords.
    ///
    /// # Errors
    /// [`SourceError`] when DataForSEO cannot be asked.
    pub fn search_intent(
        context: &SourceContext<'_>,
        keywords: &[String],
        market: &Market,
    ) -> Result<Fetched, SourceError> {
        let keywords = &keywords[..keywords.len().min(Self::BATCH)];
        let (results, cost) = Self::post(
            context,
            "dataforseo_labs/google/search_intent/live",
            &json!({ "keywords": keywords, "language_code": market.language }),
        )?;
        let keywords: Vec<FoundKeyword> = results
            .iter()
            .flat_map(|result| {
                result
                    .get("items")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            })
            .filter_map(|item| {
                Some(FoundKeyword {
                    intent: label(item.pointer("/keyword_intent/label")?),
                    secondary_intent: item
                        .get("secondary_keyword_intents")
                        .and_then(Value::as_array)
                        .and_then(|intents| intents.first())
                        .and_then(|intent| intent.get("label"))
                        .and_then(label),
                    ..FoundKeyword::named(item.get("keyword")?.as_str()?)
                })
            })
            .collect();
        let estimate = Self::labs_estimate(u32::try_from(keywords.len()).unwrap_or(u32::MAX));
        Ok(Self::fetched(keywords, cost, estimate))
    }

    /// Google autocomplete suggestions for a phrase.
    ///
    /// # Errors
    /// [`SourceError`] when DataForSEO cannot be asked.
    pub fn autocomplete(
        context: &SourceContext<'_>,
        phrase: &str,
        market: &Market,
    ) -> Result<Fetched, SourceError> {
        let (results, cost) = Self::post(
            context,
            "serp/google/autocomplete/live/advanced",
            &json!({
                "keyword": phrase,
                "location_name": Self::location(market)?,
                "language_code": market.language,
            }),
        )?;
        let keywords = results
            .iter()
            .flat_map(|result| {
                result
                    .get("items")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            })
            .filter_map(|item| {
                let suggestion = item.get("suggestion")?.as_str()?;
                Some(FoundKeyword::named(suggestion))
            })
            .collect();
        Ok(Self::fetched(keywords, cost, prices::AUTOCOMPLETE))
    }
}

fn label(value: &Value) -> Option<String> {
    let label = value.as_str()?.to_ascii_lowercase();
    matches!(
        label.as_str(),
        "informational" | "navigational" | "commercial" | "transactional"
    )
    .then_some(label)
}

fn monthly(value: Option<&Value>) -> Vec<Monthly> {
    let mut months: Vec<Monthly> = value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let year = count(item.get("year")?)?;
            let month = count(item.get("month")?)?;
            Some(Monthly {
                month: format!("{year:04}-{month:02}"),
                volume: count(item.get("search_volume")?)?,
            })
        })
        .collect();
    months.sort_by(|a, b| a.month.cmp(&b.month));
    months
}

/// A Keywords Data (Google Ads) result.
fn ads_item(item: &Value) -> Option<FoundKeyword> {
    Some(FoundKeyword {
        volume: item.get("search_volume").and_then(count),
        cpc_usd: item.get("cpc").and_then(Value::as_f64),
        competition: item
            .get("competition_index")
            .and_then(Value::as_f64)
            .map(|index| index / 100.0),
        monthly: monthly(item.get("monthly_searches")),
        ..FoundKeyword::named(item.get("keyword")?.as_str()?)
    })
}

/// A Labs item: the keyword data itself or under `keyword_data`, with an optional ranking.
fn labs_item(item: &Value) -> Option<FoundKeyword> {
    let data = item.get("keyword_data").unwrap_or(item);
    let info = data.get("keyword_info");
    let difficulty = data
        .pointer("/keyword_properties/keyword_difficulty")
        .or_else(|| item.get("keyword_difficulty"))
        .and_then(Value::as_f64);
    let ranked = item.pointer("/ranked_serp_element/serp_item");
    Some(FoundKeyword {
        volume: info
            .and_then(|info| info.get("search_volume"))
            .and_then(count),
        cpc_usd: info
            .and_then(|info| info.get("cpc"))
            .and_then(Value::as_f64),
        competition: info
            .and_then(|info| info.get("competition"))
            .and_then(Value::as_f64),
        difficulty,
        monthly: monthly(info.and_then(|info| info.get("monthly_searches"))),
        intent: data
            .pointer("/search_intent_info/main_intent")
            .and_then(label),
        secondary_intent: data
            .pointer("/search_intent_info/foreign_intent/0")
            .and_then(label),
        url: ranked
            .and_then(|serp| serp.get("url"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        position: ranked
            .and_then(|serp| serp.get("rank_group"))
            .and_then(count)
            .and_then(|rank| u32::try_from(rank).ok()),
        ..FoundKeyword::named(data.get("keyword")?.as_str()?)
    })
}
