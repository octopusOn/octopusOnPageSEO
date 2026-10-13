use duckdb::Connection;

use super::*;
use crate::site::tests::{Fixture, GOOGLEBOT, file, hit_batch_tagged, range, record};
use crate::site::{ImportOptions, SiteStore};

/// Verifies by a fixed answer.
struct Answer(&'static str, &'static str);

impl BotIpVerifier for Answer {
    fn verify(&self, pairs: &[BotIpPair]) -> Vec<BotIpVerdict> {
        vec![
            BotIpVerdict {
                verdict: self.0,
                method: self.1,
            };
            pairs.len()
        ]
    }
}

const VERIFIED: Answer = Answer("verified", "ip-list");
const SPOOFED: Answer = Answer("spoofed", "ip-list");
const SKIPPED: Answer = Answer("unverified", "dns-skipped");

/// Tags Googlebot user agents, at a given rules version.
struct Rules(u32);

impl UserAgentClassifier for Rules {
    fn version(&self) -> u32 {
        self.0
    }

    fn classify(&self, user_agents: &[String]) -> Vec<Option<BotTag>> {
        user_agents
            .iter()
            .map(|ua| ua.contains("Googlebot").then_some(GOOGLEBOT))
            .collect()
    }
}

fn import(store: &mut SiteStore, tagged: bool, verifier: &dyn BotIpVerifier) {
    let source = file("a.log");
    let mut session = store
        .begin_import(ImportOptions::default(), verifier)
        .unwrap();
    session
        .append(hit_batch_tagged(&range(0, 100), tagged))
        .unwrap();
    session.commit(&record(&source)).unwrap();
}

/// Hits per verification value in the day rollup.
fn rollup_verification(connection: &Connection) -> Vec<(Option<String>, u64)> {
    let mut statement = connection
        .prepare(
            "SELECT verification, sum(hits)::UBIGINT FROM r_day_url GROUP BY 1 ORDER BY 1 NULLS FIRST",
        )
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn imports_store_bots_verdicts_and_split_rollups() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    import(&mut store, true, &VERIFIED);

    let bot: Option<String> = store
        .connection
        .query_row(
            "SELECT bot FROM user_agents WHERE raw LIKE 'Googlebot%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(bot.as_deref(), Some("googlebot-desktop"));
    assert_eq!(store.verdict_counts().unwrap(), [("verified".into(), 1)]);
    assert_eq!(
        rollup_verification(&store.connection),
        [(None, 50), (Some("verified".into()), 50)]
    );
}

#[test]
fn reverification_rebuilds_only_changed_bot_rows() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    import(&mut store, true, &SKIPPED);

    let pending = store.verify_bots(&SKIPPED, VerifyScope::All).unwrap();
    assert_eq!((pending.checked, pending.changed), (1, 0));
    assert_eq!(pending.rebuilt_bots, Vec::<String>::new());

    let outcome = store.verify_bots(&SPOOFED, VerifyScope::Pending).unwrap();
    assert_eq!(outcome.rebuilt_bots, ["googlebot-desktop"]);
    assert_eq!(
        rollup_verification(&store.connection),
        [(None, 50), (Some("spoofed".into()), 50)]
    );
    assert_eq!(store.totals().unwrap().hits, 100);
    // Spoofed by IP list is final, so a pending-only pass leaves it alone.
    let again = store.verify_bots(&VERIFIED, VerifyScope::Pending).unwrap();
    assert_eq!(again.checked, 0);
}

#[test]
fn new_rules_reclassify_stored_user_agents() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    import(&mut store, false, &VERIFIED);
    assert_eq!(rollup_verification(&store.connection), [(None, 100)]);

    let outcome = store.reclassify_user_agents(&Rules(1), &VERIFIED).unwrap();
    assert!(outcome.performed);
    assert_eq!((outcome.changed_user_agents, outcome.new_pairs), (1, 1));
    assert_eq!(
        rollup_verification(&store.connection),
        [(None, 50), (Some("verified".into()), 50)]
    );
    let import_ids: i64 = store
        .connection
        .query_row("SELECT min(import_id) FROM r_day_url", [], |row| row.get(0))
        .unwrap();
    assert_eq!(import_ids, 1, "import ids are recovered from file names");

    let unchanged = store.reclassify_user_agents(&Rules(1), &SPOOFED).unwrap();
    assert!(!unchanged.performed);
    drop(store);
    let reader = crate::SiteReader::open(
        &fixture.data,
        crate::SiteId::new(1),
        crate::StoreSettings::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(reader.bot_catalog_version().unwrap(), 1);
}
