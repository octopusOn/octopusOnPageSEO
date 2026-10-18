use super::*;

#[test]
fn lists_and_filters_the_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::at(dir.path());
    let all = list_bots(&workspace, None);
    assert!(all.len() > 80);
    let ai = list_bots(&workspace, Some("ai-training"));
    assert!(ai.iter().any(|bot| bot.id == "gptbot"));
    assert!(ai.iter().all(|bot| bot.category == "ai-training"));
    assert_eq!(bot_categories().len(), 9);
    let google = all.iter().find(|b| b.id == "googlebot-smartphone").unwrap();
    assert_eq!(google.verification, "ip-list+dns");
}

#[test]
fn identifies_user_agents() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::at(dir.path());
    let bot = identify_user_agent(&workspace, "Mozilla/5.0 (compatible; bingbot/2.0)").unwrap();
    assert_eq!(
        (bot.id, bot.operator, bot.custom),
        ("bingbot", "Microsoft", false)
    );
    assert_eq!(
        identify_user_agent(&workspace, "Mozilla/5.0 (Windows NT 10.0) Firefox/141.0"),
        None
    );
}

#[test]
fn custom_bots_classify_and_verify_existing_and_new_hits() {
    use crate::test_support::Fixture;
    let fixture = Fixture::imported();
    fixture.import(
        "monitor.log",
        r#"203.0.113.5 - - [09/Oct/2026:10:00:00 +0000] "GET /health HTTP/1.1" 200 2 "-" "AcmeMonitor/1.0"
198.51.100.9 - - [09/Oct/2026:10:00:01 +0000] "GET /health HTTP/1.1" 200 2 "-" "AcmeMonitor/1.0"
"#,
    );
    // Unknown before it is defined.
    let (_, before) = fixture.run("ua:AcmeMonitor* group:bot");
    assert_eq!(before.rows[0][0], octopuson_core::table::Cell::Null);

    save_custom_bots(
        &fixture.workspace,
        vec![CustomBot {
            id: "acme-monitor".into(),
            name: "Acme Monitor".into(),
            category: "monitoring".into(),
            patterns: vec!["acmemonitor".into()],
            ip_ranges: vec!["203.0.113.0/24".into()],
            ..CustomBot::default()
        }],
    )
    .unwrap();
    let listed = list_bots(&fixture.workspace, Some("monitoring"));
    assert!(
        listed
            .iter()
            .any(|bot| bot.id == "acme-monitor" && bot.custom)
    );
    assert_eq!(
        identify_user_agent(&fixture.workspace, "AcmeMonitor/2").map(|b| b.id),
        Some("acme-monitor")
    );

    // Stored hits are re-classified on the next query; verdicts follow on verification.
    let (_, after) = fixture.run("bot:acme-monitor group:bot_category metric:hits");
    assert_eq!(
        after.rows[0][0],
        octopuson_core::table::Cell::Text("monitoring".into())
    );
    verify_site_bots(
        &fixture.workspace,
        &VerifySiteRequest {
            site: "acme.com".into(),
            dns: false,
            all: true,
        },
    )
    .unwrap();
    let (_, verdicts) =
        fixture.run("bot:acme-monitor group:verification metric:hits sort:verification:asc");
    let cells: Vec<_> = verdicts
        .rows
        .iter()
        .map(|row| (row[0].clone(), row[1].clone()))
        .collect();
    assert_eq!(
        cells,
        [
            (
                octopuson_core::table::Cell::Text("spoofed".into()),
                octopuson_core::table::Cell::Integer(1)
            ),
            (
                octopuson_core::table::Cell::Text("verified".into()),
                octopuson_core::table::Cell::Integer(1)
            ),
        ]
    );
    let check = verify_ip(&fixture.workspace, "203.0.113.200", "acme-monitor", false).unwrap();
    assert_eq!((check.verdict, check.method), ("verified", "ip-list"));
}

#[test]
fn verifies_single_ips_against_bundled_lists() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::at(dir.path());
    assert!(matches!(
        verify_ip(&workspace, "1.2.3.4", "nobot", false),
        Err(BotError::UnknownBot(_))
    ));
    assert!(matches!(
        verify_ip(&workspace, "x", "gptbot", false),
        Err(BotError::InvalidIp(_))
    ));
    let check = verify_ip(&workspace, "1.2.3.4", "bytespider", false).unwrap();
    assert_eq!(
        (check.verdict, check.method),
        ("unverified", "not-verifiable")
    );
}
