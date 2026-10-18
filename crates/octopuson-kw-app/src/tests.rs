use std::path::Path;

use octopuson_keywords::{Level, Mode};
use octopuson_kw_store::{
    KeywordQuery, KeywordSort, KeywordStatus, Pin, ProjectSpec, SerpRecord, SitePageRecord,
};
use octopuson_secrets::{FileBackend, Secrets};

use crate::cluster::ClusterRequest;
use crate::settings::{Method, Settings};
use crate::{KeywordWorkspace, KwAppError};

fn workspace() -> (tempfile::TempDir, KeywordWorkspace) {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Secrets::new(
        Box::new(FileBackend::new(dir.path().join("secrets.json"))),
        dir.path().join("secret-bindings.json"),
    );
    let workspace = KeywordWorkspace::with_secrets(dir.path().to_owned(), secrets);
    (dir, workspace)
}

fn spec() -> ProjectSpec {
    ProjectSpec {
        name: "  Kahve  ".to_owned(),
        country: "TR".to_owned(),
        language: "tr".to_owned(),
        engines: vec![
            "google".to_owned(),
            "Google".to_owned(),
            "yandex".to_owned(),
        ],
        site: Some("https://www.Ornek.com.tr/".to_owned()),
        competitors: vec![
            "rakip.com".to_owned(),
            "ornek.com.tr".to_owned(),
            String::new(),
        ],
    }
}

fn write(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn settings_default_and_refuse_out_of_range_values() {
    let (_dir, workspace) = workspace();
    let mut settings = workspace.settings().unwrap();
    assert_eq!(settings, Settings::default());
    assert!(!settings.suggest.google && !settings.suggest.yandex);

    settings.suggest.google = true;
    assert!(matches!(
        workspace.save_settings(&settings),
        Err(KwAppError::InvalidSettings(_))
    ));
    settings.suggest.accepted_at = Some("2026-10-18T10:00:00Z".to_owned());
    settings.suggest.daily_limit = 5_000;
    assert!(workspace.save_settings(&settings).is_err());
    settings.suggest.daily_limit = 500;
    settings.suggest.pause_ms = 500;
    assert!(workspace.save_settings(&settings).is_err());
    settings.suggest.pause_ms = 3_000;
    settings.budgets.insert("dataforseo".to_owned(), 5.0);
    workspace.save_settings(&settings).unwrap();
    assert_eq!(workspace.settings().unwrap(), settings);
}

#[test]
fn projects_are_checked_and_normalised() {
    let (_dir, workspace) = workspace();
    let project = workspace.create_project(&spec()).unwrap();
    assert_eq!(project.spec.name, "Kahve");
    assert_eq!(project.spec.country, "tr");
    assert_eq!(project.spec.engines, ["google", "yandex"]);
    assert_eq!(project.spec.site.as_deref(), Some("ornek.com.tr"));
    assert_eq!(project.spec.competitors, ["rakip.com"]);

    let mut bad = spec();
    bad.engines = vec!["altavista".to_owned()];
    assert!(matches!(
        workspace.create_project(&bad),
        Err(KwAppError::Invalid(_))
    ));
    bad = spec();
    bad.country = "TUR".to_owned();
    assert!(workspace.create_project(&bad).is_err());
    bad = spec();
    bad.name = " ".to_owned();
    assert!(workspace.create_project(&bad).is_err());
    assert_eq!(workspace.projects().unwrap().len(), 1);
    workspace.delete_project(project.id).unwrap();
    assert_eq!(workspace.projects().unwrap(), Vec::new());
}

#[test]
fn added_keywords_get_identities_and_intents() {
    let (_dir, workspace) = workspace();
    let project = workspace.create_project(&spec()).unwrap();
    let texts: Vec<String> = [
        "Türk Kahvesi Fiyatları",
        "turk kahvesi fiyatlari",
        "türk kahvesi nasıl yapılır",
        "!!!",
        "",
    ]
    .iter()
    .map(|text| (*text).to_owned())
    .collect();
    let outcome = workspace
        .add_keywords(project.id, &texts, "manual", None)
        .unwrap();
    assert_eq!(outcome.added.added, 2);
    assert_eq!(outcome.rejected, 2);
    let page = workspace
        .keywords(
            project.id,
            &KeywordQuery {
                search: Some("FİYAT".to_owned()),
                ..KeywordQuery::default()
            },
        )
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.rows[0].intent.as_deref(), Some("transactional"));
    let all = workspace
        .keywords(
            project.id,
            &KeywordQuery {
                sort: KeywordSort::Added,
                descending: false,
                ..KeywordQuery::default()
            },
        )
        .unwrap();
    assert_eq!(all.rows[1].intent.as_deref(), Some("informational"));
}

#[test]
fn imports_exports_of_keyword_tools() {
    let (dir, workspace) = workspace();
    let project = workspace.create_project(&spec()).unwrap();
    let ahrefs = write(
        dir.path(),
        "ahrefs.csv",
        "Keyword,Volume,KD,CPC,Global volume\n\
         türk kahvesi,\"74,000\",42,0.35,90000\n\
         Türk Kahvesi,1,1,1,1\n\
         filtre kahve,\"12,100\",18,0.5,15000\n",
    );
    let preview = workspace.preview_import(&ahrefs).unwrap();
    assert_eq!(preview.total, 3);
    assert_eq!(preview.columns[4].role, None);
    let outcome = workspace.import(project.id, &ahrefs).unwrap();
    assert_eq!(outcome.rows, 3);
    assert_eq!(outcome.keywords.added.added, 2);
    assert_eq!(outcome.metrics, 3);

    let list = write(dir.path(), "list.txt", "kahve makinesi\nfiltre kahve\n\n");
    let outcome = workspace.import(project.id, &list).unwrap();
    assert_eq!(outcome.keywords.added.added, 1);
    assert_eq!(outcome.keywords.added.known, 1);

    let page = workspace
        .keywords(project.id, &KeywordQuery::default())
        .unwrap();
    let rows: Vec<_> = page
        .rows
        .iter()
        .map(|row| (row.keyword.as_str(), row.volume, row.difficulty))
        .collect();
    // The last row of one keyword wins.
    assert_eq!(
        rows,
        [
            ("filtre kahve", Some(12_100), Some(18.0)),
            ("türk kahvesi", Some(1), Some(1.0)),
            ("kahve makinesi", None, None),
        ]
    );
    assert_eq!(page.rows[0].sources, ["import"]);

    assert!(matches!(
        workspace.import(project.id, &write(dir.path(), "x.pdf", "")),
        Err(KwAppError::Invalid(_))
    ));
}

#[test]
fn clusters_keep_manual_placements_and_export() {
    let (dir, workspace) = workspace();
    let project = workspace.create_project(&spec()).unwrap();
    let texts: Vec<String> = [
        "kahve makinesi",
        "kahve makineleri",
        "en iyi kahve makinesi",
        "türk kahvesi",
        "türk kahvesi nasıl yapılır",
        "çay demliği",
    ]
    .iter()
    .map(|text| (*text).to_owned())
    .collect();
    workspace
        .add_keywords(project.id, &texts, "manual", None)
        .unwrap();
    let request = ClusterRequest {
        method: Method::Lexical,
        threshold: 0.5,
        ..ClusterRequest::default()
    };
    let first = workspace.cluster(project.id, &request).unwrap();
    assert_eq!(first.clustering.keywords, 6);
    assert_eq!(first.verified, 0);
    let rows = workspace
        .keywords(project.id, &KeywordQuery::default())
        .unwrap()
        .rows;
    let cluster_of = |text: &str| rows.iter().find(|row| row.keyword == text).unwrap().cluster;
    assert_eq!(cluster_of("kahve makinesi"), cluster_of("kahve makineleri"));
    assert_ne!(cluster_of("kahve makinesi"), cluster_of("çay demliği"));

    // The tea pot is placed with the coffee machines by hand and stays there.
    let id_of = |text: &str| rows.iter().find(|row| row.keyword == text).unwrap().id;
    workspace
        .store()
        .unwrap()
        .set_pins(
            project.id,
            &[Pin {
                keyword_id: id_of("çay demliği"),
                pillar_id: id_of("kahve makinesi"),
            }],
        )
        .unwrap();
    let second = workspace.cluster(project.id, &request).unwrap();
    assert_eq!(second.placed, 1);
    let rows = workspace
        .keywords(project.id, &KeywordQuery::default())
        .unwrap()
        .rows;
    let cluster_of = |text: &str| rows.iter().find(|row| row.keyword == text).unwrap().cluster;
    assert_eq!(cluster_of("çay demliği"), cluster_of("kahve makinesi"));

    let csv = dir.path().join("clusters.csv");
    assert_eq!(
        workspace.export_clusters(project.id, None, &csv).unwrap(),
        6
    );
    let text = std::fs::read_to_string(&csv).unwrap();
    assert!(text.starts_with("cluster,cluster_label,"));
    assert!(text.contains("çay demliği"));
    let json = dir.path().join("keywords.json");
    assert_eq!(
        workspace
            .export_keywords(project.id, &KeywordQuery::default(), &json)
            .unwrap(),
        6
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 6);
    let xlsx = dir.path().join("clusters.xlsx");
    assert_eq!(
        workspace.export_clusters(project.id, None, &xlsx).unwrap(),
        6
    );
}

#[test]
fn result_pages_verify_clusters_and_map_the_site() {
    let (_dir, workspace) = workspace();
    let project = workspace.create_project(&spec()).unwrap();
    let texts: Vec<String> = ["kahve makinesi", "espresso cihazı", "çay demliği"]
        .iter()
        .map(|text| (*text).to_owned())
        .collect();
    workspace
        .add_keywords(project.id, &texts, "manual", None)
        .unwrap();
    let ids = workspace.store().unwrap().key_index(project.id).unwrap();
    let id = |key: &str| ids[key];
    let serp = |keyword_id: i64, urls: &[&str]| SerpRecord {
        keyword_id,
        engine: "google".to_owned(),
        device: "desktop".to_owned(),
        provider: "test".to_owned(),
        fetched_at: "2026-10-18T10:00:00Z".to_owned(),
        urls: urls.iter().map(|url| format!("https://{url}")).collect(),
        site_only: false,
        features: Vec::new(),
        questions: Vec::new(),
        related: Vec::new(),
        raw_path: None,
    };
    let shared = ["a.com/1", "b.com/1", "c.com/1", "rakip.com/x"];
    {
        let mut store = workspace.store().unwrap();
        store
            .put_serps(&[
                serp(id("kahve makinesi"), &shared),
                serp(id("espresso cihazi"), &shared),
                serp(id("cay demligi"), &["t.com/1", "u.com/1"]),
            ])
            .unwrap();
        store
            .put_site_pages(
                project.id,
                "gsc",
                &[SitePageRecord {
                    keyword_id: id("kahve makinesi"),
                    url: "https://ornek.com.tr/kahve-makinesi".to_owned(),
                    source: "gsc".to_owned(),
                    clicks: Some(3.0),
                    impressions: Some(120.0),
                    position: Some(8.0),
                    period: "2026-09-01..2026-09-30".to_owned(),
                }],
            )
            .unwrap();
    }
    let outcome = workspace
        .cluster(
            project.id,
            &ClusterRequest {
                method: Method::Hybrid,
                mode: Mode::Hard,
                level: Level::Page,
                ..ClusterRequest::default()
            },
        )
        .unwrap();
    assert_eq!(outcome.with_serp, 3);
    assert_eq!(outcome.clustering.clusters, 2);
    let store = workspace.store().unwrap();
    let clusters = store.cluster_records(outcome.clustering.id).unwrap();
    let machines = clusters.iter().find(|cluster| cluster.size == 2).unwrap();
    assert!(machines.verified);
    assert_eq!(
        machines.target_url.as_deref(),
        Some("https://ornek.com.tr/kahve-makinesi")
    );
    assert!(!machines.gap);
    assert_eq!(machines.competitors.len(), 4);
    let tea = clusters.iter().find(|cluster| cluster.size == 1).unwrap();
    assert!(tea.gap);

    // Excluded keywords are left out.
    drop(store);
    workspace
        .set_status(project.id, &[id("cay demligi")], KeywordStatus::Excluded)
        .unwrap();
    let again = workspace
        .cluster(project.id, &ClusterRequest::default())
        .unwrap();
    assert_eq!(again.clustering.keywords, 2);
}
