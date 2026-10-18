use super::*;
use octopuson_core::{
    Date,
    table::{Cell, Column, ColumnKind, TableSink},
};
fn project() -> OpportunityProject {
    OpportunityProject {
        id: "fixture".into(),
        name: "Fixture".into(),
        site_url: "https://example.com/".into(),
        ..Default::default()
    }
}
fn request(path: &Path) -> ImportRequest {
    ImportRequest {
        project_id: "fixture".into(),
        path: path.to_string_lossy().into(),
        sheet: None,
        source: DataSource::Gsc,
        grain: DataGrain::Pages,
        range: DateRange {
            start: Date::parse("2026-09-01").unwrap(),
            end: Date::parse("2026-09-28").unwrap(),
        },
        columns: BTreeMap::new(),
        locale: "tr".into(),
        timezone: String::new(),
        currency: None,
        google_organic: false,
        origin_confirmed: false,
    }
}
#[test]
fn turkish_csv_numbers_quoted_multiline_duplicates_and_preview_binding() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gsc.csv");
    std::fs::write(&path,"Sayfa;Tıklamalar;Gösterimler;Konum\nhttps://example.com/tr/?p=2;50;1.250;8,5\nhttps://example.com/tr/?p=2;50;1.250;8,5\n").unwrap();
    let req = request(&path);
    let (preview, dataset) = parse(&req, &project()).unwrap();
    assert_eq!(preview.valid, 1);
    assert_eq!(preview.duplicates, 1);
    let row = &dataset.unwrap().rows[0];
    assert_eq!(row.impressions, Some(1250.0));
    assert_eq!(row.position, Some(8.5));
    let mut explicit = req.clone();
    explicit.columns = preview.columns.clone();
    assert_eq!(preview.digest, preview_import_digest(&explicit));
    explicit.range.start = explicit.range.start.add_days(1);
    assert_ne!(preview.digest, preview_import_digest(&explicit));
}
fn preview_import_digest(req: &ImportRequest) -> String {
    preview(req, &project()).unwrap().digest
}
#[test]
fn english_csv_and_separate_queries_never_gain_a_page() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gsc.csv");
    std::fs::write(
        &path,
        "Query,Clicks,Impressions,Position\n\"a, b\",50,\"1,250\",8.5\n",
    )
    .unwrap();
    let mut req = request(&path);
    req.locale = "en".into();
    req.grain = DataGrain::Queries;
    let (p, data) = parse(&req, &project()).unwrap();
    assert_eq!(p.valid, 1);
    let row = &data.unwrap().rows[0];
    assert_eq!(row.url, None);
    assert_eq!(row.query.as_deref(), Some("a, b"));
    assert_eq!(row.impressions, Some(1250.0));
    req.grain = DataGrain::QueryPages;
    assert!(
        preview(&req, &project())
            .unwrap()
            .missing
            .contains(&"url".into())
    );
}
#[test]
fn missing_daily_dates_negative_metrics_and_incompatible_grains_fail() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gsc.csv");
    std::fs::write(
        &path,
        "Date,Clicks,Impressions\n,5,100\n2026-09-01,-1,200\n2026-10-01,5,100\n",
    )
    .unwrap();
    let mut req = request(&path);
    req.grain = DataGrain::Daily;
    let p = preview(&req, &project()).unwrap();
    assert_eq!(p.invalid, 3);
    req.grain = DataGrain::LandingPages;
    assert!(preview(&req, &project()).is_err());
}
#[test]
fn xlsx_cached_numbers_and_xml_entities_survive_turkish_locale() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gsc.xlsx");
    let file = std::fs::File::create(&path).unwrap();
    let mut writer = octopuson_report::XlsxWriter::new(file);
    writer
        .begin(&[
            Column::new("Page", ColumnKind::Text),
            Column::new("Clicks", ColumnKind::Float),
            Column::new("Impressions", ColumnKind::Float),
            Column::new("Position", ColumnKind::Float),
        ])
        .unwrap();
    writer
        .row(&[
            Cell::Text("https://example.com/tr/?a=1&b=2".into()),
            Cell::Float(20.0),
            Cell::Float(1250.0),
            Cell::Float(8.5),
        ])
        .unwrap();
    writer.finish().unwrap();
    drop(writer);
    let (p, data) = parse(&request(&path), &project()).unwrap();
    assert_eq!(p.valid, 1);
    let row = &data.unwrap().rows[0];
    assert_eq!(row.position, Some(8.5));
    assert_eq!(row.url.as_deref(), Some("https://example.com/tr/?a=1&b=2"));
}
#[test]
fn ga4_relative_urls_preserve_queries_and_rate_and_never_assume_origin() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ga.csv");
    std::fs::write(&path,"Landing page,Sessions,Session key event rate,Purchase revenue\n/tr/?page=2,120,2.5%,30.5\n").unwrap();
    let mut req = request(&path);
    req.source = DataSource::Ga4;
    req.grain = DataGrain::LandingPages;
    req.locale = "en".into();
    let (p, data) = parse(&req, &project()).unwrap();
    assert_eq!(p.valid, 1);
    let data = data.unwrap();
    assert_eq!(data.rows[0].goal_rate, Some(0.025));
    assert_eq!(
        data.rows[0].url.as_deref(),
        Some("https://example.com/tr/?page=2")
    );
    assert!(!data.coverage.origin_confirmed);
}

#[test]
fn custom_headers_allow_manual_mapping_and_changed_property_invalidates_preview() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("custom.csv");
    std::fs::write(
        &path,
        "Address,Click count,Seen,Rank\nhttps://example.com/a,10,2000,8\n",
    )
    .unwrap();
    let mut req = request(&path);
    assert_eq!(preview(&req, &project()).unwrap().missing.len(), 3);
    req.columns = BTreeMap::from([
        ("url".into(), 0),
        ("clicks".into(), 1),
        ("impressions".into(), 2),
        ("position".into(), 3),
    ]);
    let p = preview(&req, &project()).unwrap();
    assert_eq!(p.valid, 1);
    let mut different = project();
    different.gsc_property = "sc-domain:example.com".into();
    assert_ne!(p.digest, preview(&req, &different).unwrap().digest);
}

#[test]
fn percentages_cannot_silently_become_clicks_and_query_page_files_cannot_be_relabelled_as_page_totals()
 {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixed.csv");
    std::fs::write(
        &path,
        "Page,Clicks,Impressions,Position\nhttps://example.com/a,1%,1000,8\n",
    )
    .unwrap();
    let req = request(&path);
    let p = preview(&req, &project()).unwrap();
    assert_eq!(p.invalid, 1);
    assert!(p.errors[0].reason.contains("Percent"));
    std::fs::write(
        &path,
        "Page,Query,Clicks,Impressions,Position\nhttps://example.com/a,topic,10,1000,8\n",
    )
    .unwrap();
    assert!(
        preview(&req, &project())
            .unwrap_err()
            .contains("Query + page")
    );
    let mut req = req;
    req.grain = DataGrain::QueryPages;
    assert_eq!(preview(&req, &project()).unwrap().valid, 1);
}

#[test]
fn percentage_rate_notation_supports_turkish_prefix_and_english_suffix() {
    for (value, locale) in [("%2,5", "tr"), ("2.5%", "en")] {
        let cell = ImportCell {
            text: value.into(),
            numeric: false,
        };
        assert_eq!(number(&cell, locale, true).unwrap(), Some(0.025));
        assert!(number(&cell, locale, false).is_err());
    }
    let malformed = ImportCell {
        text: "2%5".into(),
        numeric: false,
    };
    assert!(number(&malformed, "en", true).is_err());
}
