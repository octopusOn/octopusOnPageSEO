use duckdb::{Connection, params};
use octopuson_core::table::Cell;

use crate::{CrawlReader, PageRequest, RemediationRequest};

const HTML: &str = "<html><head><title>Links</title></head><body><a href='/same' aria-label='WhatsApp'><svg aria-hidden='true'><path/></svg></a><a href='/same'><svg><path/></svg></a><a href='/title' title='Brochure'><svg aria-hidden='true'/></a><a href='/image'><img alt='Company logo'></a><a href='/generic' aria-label='Read our SEO guide'>Read more</a></body></html>";

fn seed(connection: &mut Connection) {
    crate::schema::migrate(connection).unwrap();
    for (id, path) in ["root", "named", "same", "title", "image", "generic"]
        .iter()
        .enumerate()
    {
        connection.execute("INSERT INTO urls(id,url,internal,crawled_at_ms,status,blocked_by_robots,content_kind,size_bytes,truncated,response_ms,hsts,noindex,nofollow,nosnippet,noarchive,noimageindex,indexability,title_count) VALUES(?,?,true,0,200,false,'html',0,false,0,false,false,false,false,false,false,'indexable',1)",params![id,format!("https://example.com/{path}")]).unwrap();
    }
    for (position, target) in ["same", "same", "title", "image", "generic"]
        .iter()
        .enumerate()
    {
        connection
            .execute(
                "INSERT INTO links VALUES(0,2,?,'hyperlink',?,false,false,false,true,?)",
                params![
                    format!("https://example.com/{target}"),
                    if position == 4 { "Read more" } else { "" },
                    position
                ],
            )
            .unwrap();
    }
    connection.execute_batch("INSERT INTO links VALUES(1,2,'https://example.com/same','hyperlink','',false,false,false,true,0); INSERT INTO audit_findings VALUES('anchor-empty','https://example.com/root'),('anchor-generic','https://example.com/root'),('anchor-empty','https://example.com/named'); UPDATE crawl_info SET value='0' WHERE key='audit_required';").unwrap();
    connection
        .execute(
            "INSERT INTO html VALUES(0,?),(1,?)",
            params![
                HTML,
                "<a href='/same' aria-label='WhatsApp'><svg aria-hidden='true'/></a>"
            ],
        )
        .unwrap();
}

fn fixture() -> CrawlReader {
    let mut connection = Connection::open_in_memory().unwrap();
    seed(&mut connection);
    CrawlReader::from_connection(connection)
}

#[test]
fn saved_svg_labels_fix_false_findings_but_keep_the_exact_unnamed_repeated_link() {
    let reader = fixture();
    let score = reader.seo_score().unwrap();
    assert_eq!(score.version, 4);
    assert_eq!(score.anchor_context_findings, 2);
    assert_eq!(score.deductions.len(), 1);
    assert_eq!(score.deductions[0].id, "anchor-empty");
    assert_eq!(score.deductions[0].count, 1);
    let detail = reader
        .remediation(&RemediationRequest {
            issue_id: "anchor-empty".into(),
            limit: 20,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(detail.urls.total, 1);
    assert_eq!(detail.evidence.total, 1);
    let value = |name: &str| {
        let i = detail
            .evidence
            .columns
            .iter()
            .position(|c| c.name == name)
            .unwrap();
        detail.evidence.rows[0][i].clone()
    };
    assert_eq!(value("position"), Cell::Integer(1));
    assert_eq!(value("has_svg"), Cell::Boolean(true));
    assert_eq!(value("name_source"), Cell::Text("missing".into()));
    assert_eq!(
        reader
            .page(&PageRequest {
                tab: "internal".into(),
                issue_id: "anchor-empty".into(),
                limit: 20,
                ..Default::default()
            })
            .unwrap()
            .total,
        1
    );
    assert_eq!(
        reader
            .detail("https://example.com/named")
            .unwrap()
            .findings
            .total,
        0
    );
    assert_eq!(
        reader
            .scalar("SELECT count(*) FROM audit_findings")
            .unwrap(),
        3
    );
    assert_eq!(
        reader
            .scalar("SELECT count(*) FROM links WHERE text=''")
            .unwrap(),
        5
    );
}

#[test]
fn unavailable_or_mismatched_saved_markup_never_invents_a_label() {
    for change in [
        "DELETE FROM html",
        "UPDATE html SET html='<a href=\"/unrelated\" aria-label=\"WhatsApp\"><svg/></a>'",
        "UPDATE urls SET truncated=true",
    ] {
        let reader = fixture();
        reader.connection.execute_batch(change).unwrap();
        let score = reader.seo_score().unwrap();
        assert_eq!(score.anchor_context_findings, 0, "{change}");
        assert_eq!(score.unverified_anchor_findings, 3, "{change}");
        assert!(
            !score.deductions.iter().any(|d| d.id == "anchor-empty"),
            "{change}"
        );
        assert!(
            score
                .categories
                .iter()
                .find(|c| c.id == "links")
                .unwrap()
                .value
                .is_none()
        );
    }
}

#[test]
fn new_recorded_names_work_without_html_and_optional_recommendations_do_not_cost_points() {
    let reader = fixture();
    reader.connection.execute_batch("DELETE FROM html; INSERT INTO link_names VALUES(1,0,'https://example.com/same','WhatsApp','','aria-label',true,false); INSERT INTO audit_findings VALUES('hreflang-missing-x-default','https://example.com/named'),('links-too-many','https://example.com/named');").unwrap();
    let score = reader.seo_score().unwrap();
    assert_eq!(score.advisory_findings, 2);
    assert_eq!(score.anchor_context_findings, 1);
    assert!(!score.deductions.iter().any(|d| matches!(
        d.id.as_str(),
        "hreflang-missing-x-default" | "links-too-many"
    )));
    assert!(
        reader
            .overview()
            .unwrap()
            .issues
            .iter()
            .any(|r| r.id == "links-too-many")
    );
}

#[test]
fn legacy_archive_recovery_uses_only_temporary_tables_and_keeps_database_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crawl.duckdb");
    let mut connection = Connection::open(&path).unwrap();
    seed(&mut connection);
    connection
        .execute_batch("DROP TABLE link_names; DELETE FROM schema_migrations WHERE version=10")
        .unwrap();
    drop(connection);
    let before = std::fs::read(&path).unwrap();
    for _ in 0..2 {
        let reader = CrawlReader::open(&path).unwrap();
        assert_eq!(reader.seo_score().unwrap().anchor_context_findings, 2);
        drop(reader);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

#[test]
fn expected_whatsapp_redirects_do_not_mask_internal_redirects_or_other_hosts() {
    let reader = fixture();
    reader.connection.execute_batch("DELETE FROM html; UPDATE urls SET url='https://wa.me/905555555555',status=302,internal=false WHERE id=2; UPDATE links SET target_url='https://wa.me/905555555555',internal=false WHERE target=2; UPDATE urls SET status=301 WHERE id=3; INSERT INTO links VALUES(0,3,'https://example.com/title','hyperlink','Page',false,false,false,true,5); INSERT INTO audit_findings VALUES('links-to-redirects','https://example.com/root'),('links-to-redirects','https://example.com/named');").unwrap();
    let score = reader.seo_score().unwrap();
    assert_eq!(score.contact_redirect_findings, 1);
    assert_eq!(
        score
            .deductions
            .iter()
            .find(|d| d.id == "links-to-redirects")
            .unwrap()
            .count,
        1
    );
    let detail = reader
        .remediation(&RemediationRequest {
            issue_id: "links-to-redirects".into(),
            limit: 20,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(detail.evidence.total, 1);
    assert_eq!(
        detail.evidence.rows[0][0].to_string(),
        "https://example.com/title"
    );
    for target in [
        "https://wa.me.evil.example/905555555555",
        "https://wa.me/not-a-phone",
        "https://example.com/905555555555",
    ] {
        reader
            .connection
            .execute(
                "UPDATE links SET target_url=? WHERE source=1",
                params![target],
            )
            .unwrap();
        assert_eq!(
            reader.seo_score().unwrap().contact_redirect_findings,
            0,
            "{target}"
        );
    }
}

#[test]
fn legacy_generic_only_links_can_use_their_recorded_explicit_label() {
    let reader = fixture();
    reader.connection.execute_batch("DELETE FROM links WHERE source=0 AND position<>4; UPDATE html SET html='<a href=\"/generic\" aria-label=\"Read our SEO guide\">Read more</a>' WHERE page=0").unwrap();
    let score = reader.seo_score().unwrap();
    assert!(!score.deductions.iter().any(|d| d.id == "anchor-generic"));
}
