use seonexus_core::table::{Cell, TableBuffer};
use seonexus_crawl::{
    CrawlConfig, CrawlLinkKind, CrawlPlan, CrawlProgress, CrawlSink, CrawlSummary, LinkRecord,
    UrlRecord,
};

use crate::{CrawlLibrary, LinkClass, LinkDirection, LinkScope, PageRequest, TableExportRequest};

const SOURCE: &str = "https://example.com/tr/";

fn fixture(library: &CrawlLibrary) -> String {
    let plan = CrawlPlan::new(CrawlConfig {
        start_urls: vec![SOURCE.into()],
        ..CrawlConfig::default()
    })
    .unwrap();
    let urls = [
        SOURCE,
        "https://example.com/second",
        "https://other.example/referrer",
        "https://example.com/missing?quote='&b=1",
        "https://example.com/timeout",
        "https://example.com/ok",
        "https://example.com/unchecked",
        "https://other.example/broken",
        "https://example.com/blocked",
        "https://example.com/redirect",
    ];
    let edges = [
        (0, 3),
        (0, 3),
        (0, 4),
        (0, 5),
        (0, 6),
        (0, 7),
        (0, 8),
        (0, 9),
        (1, 3),
        (1, 0),
        (2, 3),
        (2, 0),
    ];
    let mut writer = library.create(&plan).unwrap();
    for (id, url) in urls.iter().enumerate() {
        if id == 6 {
            continue;
        }
        let id = u32::try_from(id).unwrap();
        let mut record = UrlRecord::new(id, (*url).into(), Some(0), ![2, 7].contains(&id));
        record.status = match id {
            3 => Some(404),
            4 | 8 => None,
            7 => Some(503),
            9 => Some(308),
            _ => Some(200),
        };
        record.error = (id == 4).then(|| "timeout".into());
        record.blocked_by_robots = id == 8;
        let mut links: Vec<_> = edges
            .iter()
            .enumerate()
            .filter(|(_, (source, _))| *source == id)
            .map(|(position, &(source, target))| LinkRecord {
                source,
                target,
                target_url: urls[target as usize].into(),
                kind: CrawlLinkKind::Hyperlink,
                text: format!("Anchor {position}"),
                name: None,
                nofollow: false,
                ugc: false,
                sponsored: false,
                internal: target != 7,
                position: u32::try_from(position).unwrap(),
            })
            .collect();
        if id == 0 {
            links.push(LinkRecord {
                source: 0,
                target: 3,
                target_url: urls[3].into(),
                kind: CrawlLinkKind::Canonical,
                text: String::new(),
                name: None,
                nofollow: false,
                ugc: false,
                sponsored: false,
                internal: true,
                position: 20,
            });
        }
        writer.url(&record, &links).unwrap();
    }
    writer
        .finish(&CrawlSummary {
            progress: CrawlProgress::default(),
            stopped: false,
            limit_reached: false,
        })
        .unwrap()
        .id
}

#[test]
fn broken_link_evidence_identifies_targets_and_sources_without_including_good_or_unverified_links()
{
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let id = fixture(&library);
    let reader = library.open(&id).unwrap();
    let request = PageRequest {
        tab: "links".into(),
        issue_id: "links-to-broken".into(),
        limit: 1,
        sort: "source".into(),
        ..PageRequest::default()
    };
    let page = reader.page(&request).unwrap();
    assert_eq!((page.total, page.rows.len()), (5, 1));
    let summary = page.link_summary.unwrap();
    assert_eq!((summary.sources, summary.destinations), (3, 2));
    let mut export = TableBuffer::default();
    assert_eq!(
        reader
            .export_selection(
                &TableExportRequest {
                    request: request.clone(),
                    columns: vec![
                        "source".into(),
                        "destination".into(),
                        "destination_status".into(),
                        "destination_error".into()
                    ]
                },
                &mut export
            )
            .unwrap(),
        5
    );
    assert!(
        export
            .rows
            .iter()
            .any(|row| row[2] == Cell::Null && row[3] == Cell::Text("timeout".into()))
    );
    assert!(export.rows.iter().all(|row| row[1]
        == Cell::Text("https://example.com/missing?quote='&b=1".into())
        || row[1] == Cell::Text("https://example.com/timeout".into())));
    let external = reader
        .page(&PageRequest {
            issue_id: "links-to-broken-external".into(),
            ..request.clone()
        })
        .unwrap();
    assert_eq!(external.total, 1);
    let redirects = reader
        .page(&PageRequest {
            issue_id: "links-to-redirects".into(),
            ..request.clone()
        })
        .unwrap();
    assert_eq!(redirects.total, 1);
    let exact = reader
        .page(&PageRequest {
            search: "quote='&b=1".into(),
            status_class: 4,
            ..request.clone()
        })
        .unwrap();
    assert_eq!(exact.total, 4);
    assert_eq!(
        reader
            .page(&PageRequest {
                search: "' OR true --".into(),
                ..request
            })
            .unwrap()
            .total,
        0
    );
}

#[test]
fn incoming_classifies_the_referrer_and_outgoing_classifies_the_destination_with_full_counts() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let id = fixture(&library);
    let reader = library.open(&id).unwrap();
    let detail = reader.detail(SOURCE).unwrap();
    let urls = reader
        .page(&PageRequest {
            tab: "internal".into(),
            search: SOURCE.into(),
            ..PageRequest::default()
        })
        .unwrap();
    let value = |name: &str| {
        &urls.rows[0][urls
            .columns
            .iter()
            .position(|column| column.name == name)
            .unwrap()]
    };
    assert_eq!(value("internal_inlinks"), &Cell::Integer(1));
    assert_eq!(value("external_inlinks"), &Cell::Integer(1));
    assert_eq!(value("internal_outlinks"), &Cell::Integer(7));
    assert_eq!(value("external_outlinks"), &Cell::Integer(1));
    assert_eq!(
        (
            detail.link_counts.incoming.internal,
            detail.link_counts.incoming.external
        ),
        (1, 1)
    );
    assert_eq!(
        (
            detail.link_counts.outgoing.internal,
            detail.link_counts.outgoing.external
        ),
        (8, 1)
    );
    let request = PageRequest {
        tab: "links".into(),
        link_scope: Some(LinkScope {
            url: SOURCE.into(),
            direction: LinkDirection::Incoming,
        }),
        link_class: Some(LinkClass::External),
        ..PageRequest::default()
    };
    let page = reader.page(&request).unwrap();
    assert_eq!(page.total, 1);
    let source = page
        .columns
        .iter()
        .position(|c| c.name == "source")
        .unwrap();
    assert_eq!(
        page.rows[0][source],
        Cell::Text("https://other.example/referrer".into())
    );
    assert_eq!(
        reader
            .page(&PageRequest {
                link_class: Some(LinkClass::Internal),
                ..request.clone()
            })
            .unwrap()
            .total,
        1
    );
    let outgoing = reader
        .page(&PageRequest {
            link_scope: Some(LinkScope {
                url: SOURCE.into(),
                direction: LinkDirection::Outgoing,
            }),
            ..request.clone()
        })
        .unwrap();
    assert_eq!(outgoing.total, 1);
    let destination = outgoing
        .columns
        .iter()
        .position(|c| c.name == "destination")
        .unwrap();
    assert_eq!(
        outgoing.rows[0][destination],
        Cell::Text("https://other.example/broken".into())
    );
    assert!(
        reader
            .page(&PageRequest {
                tab: "internal".into(),
                link_scope: None,
                ..request
            })
            .is_err()
    );
    assert!(
        serde_json::from_str::<PageRequest>(r#"{"tab":"links","link_class":"invalid"}"#).is_err()
    );
}
