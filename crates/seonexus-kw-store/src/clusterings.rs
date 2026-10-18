//! Clusterings with their clusters and members, and manual placements kept across them.

use duckdb::types::Value;
use duckdb::{Connection, params, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::keywords::NUMBERS;
use crate::{KEPT_CLUSTERINGS, KwStore, KwStoreError, MAX_PAGE, json, parse, stage_ids};

/// How a clustering was made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusteringSpec {
    /// `lexical`, `serp` or `hybrid`.
    pub method: String,
    /// Thresholds, mode, level and the like.
    pub params: serde_json::Value,
    /// RFC 3339.
    pub created_at: String,
}

/// A stored clustering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusteringRow {
    /// Clustering id.
    pub id: i64,
    /// Its project.
    pub project_id: i64,
    /// How it was made.
    #[serde(flatten)]
    pub spec: ClusteringSpec,
    /// Keywords clustered.
    pub keywords: u32,
    /// Clusters.
    pub clusters: u32,
}

/// A competitor domain ranking for a cluster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Competitor {
    /// Registrable domain.
    pub domain: String,
    /// Member keywords it ranks for.
    pub keywords: u32,
}

/// One cluster with its totals and site mapping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterRecord {
    /// Number within the clustering, from 0.
    pub number: u32,
    /// The pillar keyword.
    pub pillar_id: i64,
    /// Label (the pillar's text unless renamed).
    pub label: String,
    /// Member keywords.
    pub size: u32,
    /// Monthly searches of all members.
    pub volume: u64,
    /// Main intent.
    pub intent: Option<String>,
    /// Whether a second intent is strong.
    pub mixed_intent: bool,
    /// Search-weighted difficulty.
    pub difficulty: Option<f64>,
    /// Search-weighted cost per click, US dollars.
    pub cpc_usd: Option<f64>,
    /// Whether result pages confirm it.
    pub verified: bool,
    /// The site's page for it.
    pub target_url: Option<String>,
    /// Other pages of the site competing for it.
    pub competing: Vec<String>,
    /// Competitors rank and the site does not.
    pub gap: bool,
    /// Domains ranking for the most members.
    pub competitors: Vec<Competitor>,
}

/// A keyword's place in a clustering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberRecord {
    /// Cluster number.
    pub number: u32,
    /// The keyword.
    pub keyword_id: i64,
    /// Similarity to the pillar, 0–1.
    pub score: f64,
    /// Results shared with the pillar, when result pages were compared.
    pub shared: Option<u16>,
}

/// A member as listed, with its keyword's numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberRow {
    /// The keyword.
    pub keyword_id: i64,
    /// Its text.
    pub keyword: String,
    /// Similarity to the pillar, 0–1.
    pub score: f64,
    /// Results shared with the pillar.
    pub shared: Option<u16>,
    /// Monthly searches.
    pub volume: Option<u64>,
    /// Cost per click, US dollars.
    pub cpc_usd: Option<f64>,
    /// Keyword difficulty.
    pub difficulty: Option<f64>,
    /// Main intent.
    pub intent: Option<String>,
    /// Whether it is the pillar.
    pub pillar: bool,
    /// Whether a manual placement put it here.
    pub pinned: bool,
}

/// Order of a cluster page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClusterSort {
    /// Monthly searches.
    #[default]
    Volume,
    /// Members.
    Size,
    /// Difficulty.
    Difficulty,
    /// Label.
    Label,
    /// Cluster number.
    Number,
}

impl ClusterSort {
    const fn column(self) -> &'static str {
        match self {
            Self::Volume => "c.volume",
            Self::Size => "c.size",
            Self::Difficulty => "c.difficulty",
            Self::Label => "lower(c.label)",
            Self::Number => "c.number",
        }
    }
}

/// Which clusters a page shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClusterQuery {
    /// Folded text one member's identity must contain.
    pub search: Option<String>,
    /// Only this intent.
    pub intent: Option<String>,
    /// Only verified or only unverified clusters.
    pub verified: Option<bool>,
    /// Only gaps, or only clusters the site ranks for.
    pub gap: Option<bool>,
    /// At least this many members.
    pub min_size: Option<u32>,
    /// Order.
    pub sort: ClusterSort,
    /// Largest first.
    pub descending: bool,
    /// Rows to skip.
    pub offset: u64,
    /// Rows to return, at most [`MAX_PAGE`].
    pub limit: u64,
}

impl Default for ClusterQuery {
    fn default() -> Self {
        Self {
            search: None,
            intent: None,
            verified: None,
            gap: None,
            min_size: None,
            sort: ClusterSort::Volume,
            descending: true,
            offset: 0,
            limit: 100,
        }
    }
}

/// A page of clusters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterPage {
    /// Clusters matching the query.
    pub total: u64,
    /// The requested rows.
    pub rows: Vec<ClusterRecord>,
}

/// A manual placement: the keyword stays with this pillar in later clusterings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pin {
    /// The keyword.
    pub keyword_id: i64,
    /// The pillar it belongs with (itself for a cluster of its own).
    pub pillar_id: i64,
}

const CLUSTER_COLUMNS: &str = "c.number, c.pillar_id, c.label, c.size, c.volume, c.intent,
    c.mixed_intent, c.difficulty, c.cpc_usd, c.verified, c.target_url, c.competing, c.gap,
    c.competitors";

impl KwStore {
    /// Saves a new clustering of a project and removes all but the newest
    /// [`KEPT_CLUSTERINGS`].
    ///
    /// # Errors
    /// Database errors.
    pub fn save_clustering(
        &mut self,
        project_id: i64,
        spec: &ClusteringSpec,
        clusters: &[ClusterRecord],
        members: &[MemberRecord],
    ) -> Result<i64, KwStoreError> {
        let transaction = self.connection.transaction()?;
        let id: i64 = transaction.query_row(
            "INSERT INTO clusterings (project_id, method, params, created_at, keywords, clusters)
             VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
            params![
                project_id,
                spec.method,
                json(&spec.params)?,
                spec.created_at,
                count(members.len()),
                count(clusters.len())
            ],
            |row| row.get(0),
        )?;
        append(&transaction, id, clusters, members)?;
        for table in ["members", "clusters"] {
            transaction.execute(
                &format!(
                    "DELETE FROM {table} WHERE clustering_id IN (
                         SELECT id FROM clusterings WHERE project_id = ? AND id NOT IN (
                             SELECT id FROM clusterings WHERE project_id = ? ORDER BY id DESC LIMIT ?
                         )
                     )"
                ),
                params![project_id, project_id, KEPT_CLUSTERINGS],
            )?;
        }
        transaction.execute(
            "DELETE FROM clusterings WHERE project_id = ? AND id NOT IN (
                 SELECT id FROM clusterings WHERE project_id = ? ORDER BY id DESC LIMIT ?
             )",
            params![project_id, project_id, KEPT_CLUSTERINGS],
        )?;
        transaction.commit()?;
        Ok(id)
    }

    /// Replaces the clusters and members of a clustering (after a manual edit).
    ///
    /// # Errors
    /// [`KwStoreError::NotFound`] for an unknown clustering; database errors.
    pub fn replace_clusters(
        &mut self,
        clustering_id: i64,
        clusters: &[ClusterRecord],
        members: &[MemberRecord],
    ) -> Result<(), KwStoreError> {
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE clusterings SET keywords = ?, clusters = ? WHERE id = ?",
            params![count(members.len()), count(clusters.len()), clustering_id],
        )?;
        if changed == 0 {
            return Err(KwStoreError::NotFound(format!(
                "clustering {clustering_id}"
            )));
        }
        transaction.execute(
            "DELETE FROM members WHERE clustering_id = ?",
            params![clustering_id],
        )?;
        transaction.execute(
            "DELETE FROM clusters WHERE clustering_id = ?",
            params![clustering_id],
        )?;
        append(&transaction, clustering_id, clusters, members)?;
        transaction.commit()?;
        Ok(())
    }

    /// Clusterings of a project, newest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn clusterings(&self, project_id: i64) -> Result<Vec<ClusteringRow>, KwStoreError> {
        self.clustering_rows("WHERE project_id = ? ORDER BY id DESC", project_id)
    }

    /// One clustering.
    ///
    /// # Errors
    /// [`KwStoreError::NotFound`] for an unknown clustering; database errors.
    pub fn clustering(&self, id: i64) -> Result<ClusteringRow, KwStoreError> {
        self.clustering_rows("WHERE id = ?", id)?
            .into_iter()
            .next()
            .ok_or_else(|| KwStoreError::NotFound(format!("clustering {id}")))
    }

    /// The newest clustering of a project.
    ///
    /// # Errors
    /// Database errors.
    pub fn latest_clustering(
        &self,
        project_id: i64,
    ) -> Result<Option<ClusteringRow>, KwStoreError> {
        Ok(self
            .clustering_rows("WHERE project_id = ? ORDER BY id DESC LIMIT 1", project_id)?
            .into_iter()
            .next())
    }

    fn clustering_rows(
        &self,
        filter: &str,
        value: i64,
    ) -> Result<Vec<ClusteringRow>, KwStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT id, project_id, method, params, created_at, keywords, clusters
             FROM clusterings {filter}"
        ))?;
        let rows = statement.query_map(params![value], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, u32>(5)?,
                row.get::<_, u32>(6)?,
            ))
        })?;
        let mut clusterings = Vec::new();
        for row in rows {
            let (id, project_id, method, params, created_at, keywords, clusters) = row?;
            clusterings.push(ClusteringRow {
                id,
                project_id,
                spec: ClusteringSpec {
                    method,
                    params: parse(&params)?,
                    created_at,
                },
                keywords,
                clusters,
            });
        }
        Ok(clusterings)
    }

    /// Deletes a clustering.
    ///
    /// # Errors
    /// [`KwStoreError::NotFound`] for an unknown clustering; database errors.
    pub fn delete_clustering(&mut self, id: i64) -> Result<(), KwStoreError> {
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM members WHERE clustering_id = ?", params![id])?;
        transaction.execute("DELETE FROM clusters WHERE clustering_id = ?", params![id])?;
        let deleted = transaction.execute("DELETE FROM clusterings WHERE id = ?", params![id])?;
        if deleted == 0 {
            return Err(KwStoreError::NotFound(format!("clustering {id}")));
        }
        transaction.commit()?;
        Ok(())
    }

    /// A page of the clusters of a clustering.
    ///
    /// # Errors
    /// Database errors, invalid stored lists.
    pub fn clusters(
        &self,
        clustering_id: i64,
        query: &ClusterQuery,
    ) -> Result<ClusterPage, KwStoreError> {
        let mut values = vec![Value::BigInt(clustering_id)];
        let mut conditions = vec!["c.clustering_id = $1".to_owned()];
        let mut bind = |value: Value| {
            values.push(value);
            format!("${}", values.len())
        };
        if let Some(search) = query.search.as_deref().filter(|text| !text.is_empty()) {
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM members m JOIN keywords k ON k.id = m.keyword_id
                         WHERE m.clustering_id = c.clustering_id AND m.number = c.number
                             AND contains(k.key, {}))",
                bind(Value::Text(search.to_owned()))
            ));
        }
        if let Some(intent) = &query.intent {
            conditions.push(format!("c.intent = {}", bind(Value::Text(intent.clone()))));
        }
        if let Some(verified) = query.verified {
            conditions.push(format!("c.verified = {}", bind(Value::Boolean(verified))));
        }
        if let Some(gap) = query.gap {
            conditions.push(format!("c.gap = {}", bind(Value::Boolean(gap))));
        }
        if let Some(size) = query.min_size {
            conditions.push(format!("c.size >= {}", bind(Value::UInt(size))));
        }
        let limit = bind(Value::UBigInt(query.limit.clamp(1, MAX_PAGE)));
        let offset = bind(Value::UBigInt(query.offset));
        let direction = if query.descending { "DESC" } else { "ASC" };
        let sql = format!(
            "SELECT {CLUSTER_COLUMNS}, count(*) OVER ()::UBIGINT FROM clusters c
             WHERE {}
             ORDER BY {} {direction} NULLS LAST, c.number
             LIMIT {limit} OFFSET {offset}",
            conditions.join(" AND "),
            query.sort.column()
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            Ok((read_cluster(row)?, row.get::<_, u64>(14)?))
        })?;
        let mut page = ClusterPage {
            total: 0,
            rows: Vec::new(),
        };
        for row in rows {
            let (cluster, total) = row?;
            page.rows.push(finish_cluster(cluster)?);
            page.total = total;
        }
        if page.rows.is_empty() && query.offset > 0 {
            let first = ClusterQuery {
                offset: 0,
                limit: 1,
                ..query.clone()
            };
            page.total = self.clusters(clustering_id, &first)?.total;
        }
        Ok(page)
    }

    /// Every cluster of a clustering, by number.
    ///
    /// # Errors
    /// Database errors, invalid stored lists.
    pub fn cluster_records(&self, clustering_id: i64) -> Result<Vec<ClusterRecord>, KwStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {CLUSTER_COLUMNS} FROM clusters c WHERE c.clustering_id = ? ORDER BY c.number"
        ))?;
        let rows = statement.query_map(params![clustering_id], read_cluster)?;
        rows.map(|row| finish_cluster(row?)).collect()
    }

    /// Every member of a clustering, by cluster and falling score.
    ///
    /// # Errors
    /// Database errors.
    pub fn member_records(&self, clustering_id: i64) -> Result<Vec<MemberRecord>, KwStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT number, keyword_id, score, shared FROM members WHERE clustering_id = ?
             ORDER BY number, score DESC, keyword_id",
        )?;
        let rows = statement.query_map(params![clustering_id], |row| {
            Ok(MemberRecord {
                number: row.get(0)?,
                keyword_id: row.get(1)?,
                score: row.get(2)?,
                shared: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Members of one cluster with their keywords' numbers, pillar first.
    ///
    /// # Errors
    /// [`KwStoreError::NotFound`] for an unknown clustering; database errors.
    pub fn cluster_members(
        &self,
        clustering_id: i64,
        number: u32,
    ) -> Result<Vec<MemberRow>, KwStoreError> {
        let project_id = self.clustering(clustering_id)?.project_id;
        let engine = self.primary_engine(project_id)?;
        let mut statement = self.connection.prepare(&format!(
            "{NUMBERS}
             SELECT k.id, k.keyword, m.score, m.shared, n.volume, n.cpc_usd, n.difficulty,
                    i.primary_intent, c.pillar_id = k.id, p.keyword_id IS NOT NULL
             FROM members m
             JOIN clusters c ON c.clustering_id = m.clustering_id AND c.number = m.number
             JOIN keywords k ON k.id = m.keyword_id
             LEFT JOIN numbers n ON n.keyword_id = k.id
             LEFT JOIN intents i ON i.keyword_id = k.id
             LEFT JOIN pins p ON p.project_id = $1 AND p.keyword_id = k.id
             WHERE m.clustering_id = $3 AND m.number = $4
             ORDER BY c.pillar_id = k.id DESC, m.score DESC, n.volume DESC NULLS LAST, k.id"
        ))?;
        let rows =
            statement.query_map(params![project_id, engine, clustering_id, number], |row| {
                Ok(MemberRow {
                    keyword_id: row.get(0)?,
                    keyword: row.get(1)?,
                    score: row.get(2)?,
                    shared: row.get(3)?,
                    volume: row.get(4)?,
                    cpc_usd: row.get(5)?,
                    difficulty: row.get(6)?,
                    intent: row.get(7)?,
                    pillar: row.get(8)?,
                    pinned: row.get(9)?,
                })
            })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Places keywords with pillars, replacing their earlier placements.
    ///
    /// # Errors
    /// Database errors.
    pub fn set_pins(&mut self, project_id: i64, pins: &[Pin]) -> Result<(), KwStoreError> {
        let ids: Vec<i64> = pins.iter().map(|pin| pin.keyword_id).collect();
        let transaction = self.connection.transaction()?;
        stage_ids(&transaction, &ids)?;
        transaction.execute(
            "DELETE FROM pins WHERE project_id = ? AND keyword_id IN (SELECT id FROM incoming_ids)",
            params![project_id],
        )?;
        transaction.execute("DELETE FROM incoming_ids", [])?;
        {
            let mut seen = std::collections::HashSet::new();
            let mut appender = transaction.appender("pins")?;
            for pin in pins.iter().rev() {
                if seen.insert(pin.keyword_id) {
                    appender.append_row(params![project_id, pin.keyword_id, pin.pillar_id])?;
                }
            }
            appender.flush()?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Removes the placements of keywords.
    ///
    /// # Errors
    /// Database errors.
    pub fn remove_pins(
        &mut self,
        project_id: i64,
        keyword_ids: &[i64],
    ) -> Result<(), KwStoreError> {
        let transaction = self.connection.transaction()?;
        stage_ids(&transaction, keyword_ids)?;
        transaction.execute(
            "DELETE FROM pins WHERE project_id = ? AND keyword_id IN (SELECT id FROM incoming_ids)",
            params![project_id],
        )?;
        transaction.execute("DELETE FROM incoming_ids", [])?;
        transaction.commit()?;
        Ok(())
    }

    /// Every placement of a project.
    ///
    /// # Errors
    /// Database errors.
    pub fn pins(&self, project_id: i64) -> Result<Vec<Pin>, KwStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT keyword_id, pillar_id FROM pins WHERE project_id = ? ORDER BY keyword_id",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok(Pin {
                keyword_id: row.get(0)?,
                pillar_id: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

fn append(
    connection: &Connection,
    clustering_id: i64,
    clusters: &[ClusterRecord],
    members: &[MemberRecord],
) -> Result<(), KwStoreError> {
    let mut appender = connection.appender("clusters")?;
    for cluster in clusters {
        appender.append_row(params![
            clustering_id,
            cluster.number,
            cluster.pillar_id,
            cluster.label,
            cluster.size,
            cluster.volume,
            cluster.intent,
            cluster.mixed_intent,
            cluster.difficulty,
            cluster.cpc_usd,
            cluster.verified,
            cluster.target_url,
            json(&cluster.competing)?,
            cluster.gap,
            json(&cluster.competitors)?
        ])?;
    }
    appender.flush()?;
    let mut appender = connection.appender("members")?;
    for member in members {
        appender.append_row(params![
            clustering_id,
            member.number,
            member.keyword_id,
            member.score,
            member.shared
        ])?;
    }
    appender.flush()?;
    Ok(())
}

fn count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

type RawCluster = (ClusterRecord, String, String);

fn read_cluster(row: &duckdb::Row<'_>) -> duckdb::Result<RawCluster> {
    Ok((
        ClusterRecord {
            number: row.get(0)?,
            pillar_id: row.get(1)?,
            label: row.get(2)?,
            size: row.get(3)?,
            volume: row.get(4)?,
            intent: row.get(5)?,
            mixed_intent: row.get(6)?,
            difficulty: row.get(7)?,
            cpc_usd: row.get(8)?,
            verified: row.get(9)?,
            target_url: row.get(10)?,
            competing: Vec::new(),
            gap: row.get(12)?,
            competitors: Vec::new(),
        },
        row.get(11)?,
        row.get(13)?,
    ))
}

fn finish_cluster(
    (mut cluster, competing, competitors): RawCluster,
) -> Result<ClusterRecord, KwStoreError> {
    cluster.competing = parse(&competing)?;
    cluster.competitors = parse(&competitors)?;
    Ok(cluster)
}
