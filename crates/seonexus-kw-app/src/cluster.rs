//! Clustering a project: wording, result pages or both, manual placements kept, totals and
//! site mapping computed, the result stored.

use std::collections::HashMap;

use seonexus_keywords::{
    Clustering, Input, KeywordFacts, Level, LexicalOptions, Mode, Placement, SerpInput,
    SerpOptions, SitePage, cluster_hybrid, cluster_lexical, cluster_serp, map_cluster, place,
    site_domain, summarize,
};
use seonexus_kw_store::{
    ClusterInput, ClusterRecord, ClusteringRow, ClusteringSpec, Competitor, MemberRecord, RunKind,
    RunStatus, SitePageRecord,
};
use seonexus_text::Language;
use serde::{Deserialize, Serialize};

use crate::keywords::{intent_name, parse_intent};
use crate::projects::ENGINES;
use crate::settings::{ClusteringDefaults, Method};
use crate::{KeywordWorkspace, KwAppError, now};

/// How to cluster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClusterRequest {
    /// Method.
    pub method: Method,
    /// Least wording similarity (0–1).
    pub threshold: f64,
    /// Least shared results (1–9).
    pub serp_threshold: u16,
    /// Soft or hard.
    pub mode: Mode,
    /// Pages or sites.
    pub level: Level,
    /// Engine whose result pages are compared; the project's primary engine when absent.
    pub engine: Option<String>,
    /// Device whose result pages are compared.
    pub device: String,
}

impl Default for ClusterRequest {
    fn default() -> Self {
        Self::from_defaults(&ClusteringDefaults::default())
    }
}

impl ClusterRequest {
    /// A request with the saved defaults.
    pub fn from_defaults(defaults: &ClusteringDefaults) -> Self {
        Self {
            method: defaults.method,
            threshold: defaults.threshold,
            serp_threshold: defaults.serp_threshold,
            mode: defaults.mode,
            level: defaults.level,
            engine: None,
            device: "desktop".to_owned(),
        }
    }

    fn check(&self) -> Result<(), KwAppError> {
        if !(self.threshold > 0.0 && self.threshold <= 1.0) {
            return Err(KwAppError::Invalid(
                "the wording threshold must be above 0 and at most 1".to_owned(),
            ));
        }
        if !(1..=9).contains(&self.serp_threshold) {
            return Err(KwAppError::Invalid(
                "the shared-results threshold must be 1–9".to_owned(),
            ));
        }
        if !matches!(self.device.as_str(), "desktop" | "mobile") {
            return Err(KwAppError::Invalid(format!(
                "unknown device `{}` (desktop or mobile)",
                self.device
            )));
        }
        Ok(())
    }
}

/// Parameters stored with a clustering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterParams {
    /// The request.
    #[serde(flatten)]
    pub request: ClusterRequest,
    /// Engine used.
    pub engine_used: String,
    /// Level used: sites when result pages held sites only.
    pub level_used: Level,
}

/// What a clustering did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterOutcome {
    /// The stored clustering.
    pub clustering: ClusteringRow,
    /// Clusters confirmed by result pages.
    pub verified: u32,
    /// Keywords with a result page that was compared.
    pub with_serp: u32,
    /// Keywords kept where the user placed them.
    pub placed: u32,
    /// Level used.
    pub level: Level,
}

fn count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

impl KeywordWorkspace {
    /// Clusters a project's active keywords and stores the clustering.
    ///
    /// Result pages of the chosen engine and device are compared for `serp` and `hybrid`;
    /// when some hold sites only (Google's hidden links), sites are compared instead of
    /// pages. Keywords the user placed by hand stay where they were placed.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for invalid options or a project without keywords; store
    /// errors.
    pub fn cluster(
        &self,
        project_id: i64,
        request: &ClusterRequest,
    ) -> Result<ClusterOutcome, KwAppError> {
        request.check()?;
        let mut store = self.store()?;
        let project = store.project(project_id)?;
        let engine = request
            .engine
            .clone()
            .or_else(|| project.spec.engines.first().cloned())
            .unwrap_or_else(|| "google".to_owned());
        if !ENGINES.contains(&engine.as_str()) {
            return Err(KwAppError::Invalid(format!("unknown engine `{engine}`")));
        }
        let inputs = store.cluster_inputs(project_id, &engine, &request.device)?;
        if inputs.is_empty() {
            return Err(KwAppError::Invalid(
                "the project has no active keywords to cluster".to_owned(),
            ));
        }
        let started = now();
        let run = store.begin_run(
            RunKind::Cluster,
            Some(project_id),
            count(inputs.len()),
            0.0,
            &started,
        )?;
        let use_serps = request.method != Method::Lexical;
        let level = if use_serps
            && inputs
                .iter()
                .any(|input| input.site_only && input.results.is_some())
        {
            Level::Site
        } else {
            request.level
        };
        let clustering = group(
            &inputs,
            request,
            Language::from_code(&project.spec.language),
            level,
        );
        let positions: HashMap<i64, usize> = inputs
            .iter()
            .enumerate()
            .map(|(index, input)| (input.id, index))
            .collect();
        let placements: Vec<Placement> = store
            .pins(project_id)?
            .iter()
            .filter_map(|pin| {
                Some(Placement {
                    keyword: *positions.get(&pin.keyword_id)?,
                    pillar: *positions.get(&pin.pillar_id)?,
                })
            })
            .collect();
        let volumes: Vec<Option<u64>> = inputs.iter().map(|input| input.volume).collect();
        let texts: Vec<&str> = inputs.iter().map(|input| input.keyword.as_str()).collect();
        let clustering = place(&clustering, &placements, &volumes, &texts);
        let site_pages = store.site_pages(project_id)?;
        let (clusters, members) = records(
            &clustering,
            &inputs,
            project.spec.site.as_deref(),
            &site_pages,
            &positions,
        );
        let params = ClusterParams {
            request: request.clone(),
            engine_used: engine,
            level_used: level,
        };
        let spec = ClusteringSpec {
            method: request.method.as_str().to_owned(),
            params: serde_json::to_value(&params)
                .map_err(|error| KwAppError::Invalid(error.to_string()))?,
            created_at: started,
        };
        let id = store.save_clustering(project_id, &spec, &clusters, &members)?;
        store.finish_run(run, RunStatus::Done, count(inputs.len()), &now(), None)?;
        Ok(ClusterOutcome {
            clustering: store.clustering(id)?,
            verified: count(clusters.iter().filter(|cluster| cluster.verified).count()),
            with_serp: if use_serps {
                count(
                    inputs
                        .iter()
                        .filter(|input| input.results.is_some())
                        .count(),
                )
            } else {
                0
            },
            placed: count(placements.len()),
            level,
        })
    }
}

/// Clusters the inputs by the requested method.
fn group(
    inputs: &[ClusterInput],
    request: &ClusterRequest,
    language: Language,
    level: Level,
) -> Clustering {
    let lexical = LexicalOptions {
        threshold: request.threshold,
        language,
    };
    let serp = SerpOptions {
        threshold: request.serp_threshold,
        mode: request.mode,
        level,
        ..SerpOptions::default()
    };
    match request.method {
        Method::Lexical => {
            let words: Vec<Input<'_>> = inputs
                .iter()
                .map(|input| Input {
                    keyword: &input.keyword,
                    volume: input.volume,
                })
                .collect();
            cluster_lexical(&words, &lexical)
        }
        Method::Serp | Method::Hybrid => {
            let pages: Vec<SerpInput<'_>> = inputs
                .iter()
                .map(|input| SerpInput {
                    keyword: &input.keyword,
                    volume: input.volume,
                    results: input.results.as_deref(),
                })
                .collect();
            if request.method == Method::Serp {
                cluster_serp(&pages, &serp)
            } else {
                cluster_hybrid(&pages, &lexical, &serp)
            }
        }
    }
}

/// Stored clusters and members of a clustering, with totals and site mapping.
fn records(
    clustering: &Clustering,
    inputs: &[ClusterInput],
    site: Option<&str>,
    site_pages: &[SitePageRecord],
    positions: &HashMap<i64, usize>,
) -> (Vec<ClusterRecord>, Vec<MemberRecord>) {
    let facts: Vec<KeywordFacts> = inputs
        .iter()
        .map(|input| KeywordFacts {
            volume: input.volume,
            difficulty: input.difficulty,
            cpc: input.cpc_usd,
            intent: input.intent.as_deref().and_then(parse_intent),
        })
        .collect();
    let results: Vec<Option<&[String]>> = inputs
        .iter()
        .map(|input| input.results.as_deref())
        .collect();
    let pages: Vec<SitePage> = site_pages
        .iter()
        .filter_map(|page| {
            Some(SitePage {
                keyword: *positions.get(&page.keyword_id)?,
                url: page.url.clone(),
                impressions: page.impressions,
                position: page.position,
            })
        })
        .collect();
    let site = site.and_then(site_domain);
    let mut clusters = Vec::with_capacity(clustering.clusters.len());
    let mut members = Vec::with_capacity(inputs.len());
    for (number, cluster) in clustering.clusters.iter().enumerate() {
        let number = count(number);
        let summary = summarize(cluster, &facts);
        // Without a site, the mapping still names the competitors.
        let map = map_cluster(cluster, &pages, &results, site.as_deref().unwrap_or(""));
        let evidence = !pages.is_empty() || cluster.indices().any(|index| results[index].is_some());
        let pillar = &inputs[cluster.pillar];
        clusters.push(ClusterRecord {
            number,
            pillar_id: pillar.id,
            label: pillar.keyword.clone(),
            size: count(cluster.len()),
            volume: summary.volume,
            intent: summary.intent.map(|intent| intent_name(intent).to_owned()),
            mixed_intent: summary.mixed_intent,
            difficulty: summary.difficulty,
            cpc_usd: summary.cpc,
            verified: cluster.verified,
            target_url: map.target,
            competing: map.competing,
            gap: site.is_some() && evidence && map.gap,
            competitors: map
                .competitors
                .into_iter()
                .map(|(domain, keywords)| Competitor { domain, keywords })
                .collect(),
        });
        members.extend(cluster.members.iter().map(|member| MemberRecord {
            number,
            keyword_id: inputs[member.index].id,
            score: member.score,
            shared: member.shared,
        }));
    }
    (clusters, members)
}

/// Maps the newest clustering again: totals and site mapping from the current numbers, site
/// pages and result pages, manual placements applied, clusters otherwise unchanged (keywords
/// excluded since are left out).
pub(crate) fn remap(
    workspace: &KeywordWorkspace,
    project_id: i64,
) -> Result<ClusteringRow, KwAppError> {
    let mut store = workspace.store()?;
    let project = store.project(project_id)?;
    let clustering = store
        .latest_clustering(project_id)?
        .ok_or_else(|| KwAppError::Invalid("the project has no clustering yet".to_owned()))?;
    let params: Option<ClusterParams> = serde_json::from_value(clustering.spec.params.clone()).ok();
    let engine = params
        .as_ref()
        .map(|params| params.engine_used.clone())
        .or_else(|| project.spec.engines.first().cloned())
        .unwrap_or_else(|| "google".to_owned());
    let device = params.map_or_else(|| "desktop".to_owned(), |params| params.request.device);
    let inputs = store.cluster_inputs(project_id, &engine, &device)?;
    let positions: HashMap<i64, usize> = inputs
        .iter()
        .enumerate()
        .map(|(index, input)| (input.id, index))
        .collect();
    let stored = store.cluster_records(clustering.id)?;
    let mut clusters: Vec<seonexus_keywords::Cluster> = stored
        .iter()
        .map(|cluster| seonexus_keywords::Cluster {
            pillar: positions
                .get(&cluster.pillar_id)
                .copied()
                .unwrap_or(usize::MAX),
            members: Vec::new(),
            verified: cluster.verified,
        })
        .collect();
    for member in store.member_records(clustering.id)? {
        let (Some(index), Some(cluster)) = (
            positions.get(&member.keyword_id),
            clusters.get_mut(member.number as usize),
        ) else {
            continue;
        };
        cluster.members.push(seonexus_keywords::Member {
            index: *index,
            score: member.score,
            shared: member.shared,
        });
    }
    let volumes: Vec<Option<u64>> = inputs.iter().map(|input| input.volume).collect();
    let texts: Vec<&str> = inputs.iter().map(|input| input.keyword.as_str()).collect();
    clusters.retain(|cluster| !cluster.is_empty());
    // Manual placements apply; a pillar that left keeps its cluster under its most searched
    // remaining member.
    let placements: Vec<Placement> = store
        .pins(project_id)?
        .iter()
        .filter_map(|pin| {
            Some(Placement {
                keyword: *positions.get(&pin.keyword_id)?,
                pillar: *positions.get(&pin.pillar_id)?,
            })
        })
        .collect();
    let regrouped =
        seonexus_keywords::place(&Clustering { clusters }, &placements, &volumes, &texts);
    let site_pages = store.site_pages(project_id)?;
    let (records, members) = records(
        &regrouped,
        &inputs,
        project.spec.site.as_deref(),
        &site_pages,
        &positions,
    );
    store.replace_clusters(clustering.id, &records, &members)?;
    Ok(store.clustering(clustering.id)?)
}

impl KeywordWorkspace {
    /// Places keywords with a pillar keyword by hand, in the newest clustering and in every
    /// later one. A pillar among `keyword_ids` leads a cluster of its own with the others
    /// (a split); keywords of another cluster moved to its pillar merge into it.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] without keywords or a clustering; store errors.
    pub fn place_keywords(
        &self,
        project_id: i64,
        keyword_ids: &[i64],
        pillar_id: i64,
    ) -> Result<ClusteringRow, KwAppError> {
        if keyword_ids.is_empty() {
            return Err(KwAppError::Invalid("choose keywords to move".to_owned()));
        }
        let mut pins: Vec<seonexus_kw_store::Pin> = keyword_ids
            .iter()
            .filter(|id| **id != pillar_id)
            .map(|keyword_id| seonexus_kw_store::Pin {
                keyword_id: *keyword_id,
                pillar_id,
            })
            .collect();
        if keyword_ids.contains(&pillar_id) {
            pins.push(seonexus_kw_store::Pin {
                keyword_id: pillar_id,
                pillar_id,
            });
        }
        self.store()?.set_pins(project_id, &pins)?;
        remap(self, project_id)
    }

    /// Forgets manual placements, so the next clustering decides again.
    ///
    /// # Errors
    /// Store errors.
    pub fn unplace_keywords(&self, project_id: i64, keyword_ids: &[i64]) -> Result<(), KwAppError> {
        Ok(self.store()?.remove_pins(project_id, keyword_ids)?)
    }
}
