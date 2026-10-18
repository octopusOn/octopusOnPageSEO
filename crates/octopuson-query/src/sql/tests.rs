use super::*;
use crate::dsl::parse;

fn ctx() -> CompileContext {
    CompileContext {
        today: Date::parse("2026-10-12").unwrap(),
    }
}

fn compiled(text: &str) -> CompiledQuery {
    compile(&parse(text).unwrap(), ctx()).unwrap()
}

#[test]
fn status_breakdown_uses_hourly_rollup() {
    let query = compiled("group:status");
    assert_eq!(query.source, Source::HourRollup);
    assert_eq!(
        query.sql,
        "SELECT CAST(t.status AS BIGINT) AS \"status\", CAST(sum(t.hits) AS BIGINT) AS \"hits\" \
         FROM r_hour t GROUP BY 1 ORDER BY 2 DESC NULLS LAST, 1 ASC NULLS LAST"
    );
    assert_eq!(query.params, []);
}

#[test]
fn url_report_uses_daily_rollup_with_url_join() {
    let query = compiled("status:4xx date:last7d group:url metric:hits,last_seen limit:10");
    assert_eq!(query.source, Source::DayUrlRollup);
    assert_eq!(
        query.sql,
        "SELECT CAST(u.raw_url AS VARCHAR) AS \"url\", CAST(sum(t.hits) AS BIGINT) AS \"hits\", \
         max(t.last_ts) AS \"last_seen\" FROM r_day_url t JOIN urls u ON u.url_id = t.url_id \
         WHERE ((t.status BETWEEN ? AND ?) AND (t.day BETWEEN ? AND ?)) \
         GROUP BY 1 ORDER BY 2 DESC NULLS LAST, 1 ASC NULLS LAST"
    );
    assert!(query.sql_for(None).ends_with("NULLS LAST LIMIT 10"));
    assert_eq!(
        query.params,
        [
            SqlValue::Integer(400),
            SqlValue::Integer(499),
            SqlValue::Date(Date::parse("2026-10-06").unwrap()),
            SqlValue::Date(Date::parse("2026-10-12").unwrap()),
        ]
    );
}

#[test]
fn per_hit_fields_fall_back_to_raw_hits() {
    let query = compiled("ip:1.2.3.4 group:url");
    assert_eq!(query.source, Source::Hits);
    assert!(query.sql.contains("FROM hits t JOIN urls u"));
    let p95 = compiled("group:url metric:p95_ms");
    assert_eq!(p95.source, Source::Hits);
    let listing = compiled("status:404");
    assert_eq!(listing.source, Source::Hits);
    assert_eq!(listing.columns.len(), crate::ROW_COLUMNS.len());
    assert!(listing.sql.ends_with("ORDER BY 1 DESC NULLS LAST"));
}

#[test]
fn raw_date_filters_prune_months_and_use_timestamp_ranges() {
    let query = compiled("date:2026-09-30..2026-10-01 ip:1.1.1.1");
    assert!(
        query
            .sql
            .contains("((TRUE AND t.month >= ? AND t.ts >= ? AND t.month <= ? AND t.ts < ?))")
    );
    assert_eq!(query.params[0], SqlValue::Text("2026-09".into()));
    assert_eq!(query.params[2], SqlValue::Text("2026-10".into()));
    let SqlValue::Timestamp(end) = query.params[3] else {
        panic!("expected a timestamp");
    };
    assert_eq!(end.to_string(), "2026-10-02T00:00:00Z");
}

#[test]
fn user_values_are_always_parameters() {
    let query = compiled("url:\"'; DROP TABLE urls; --\" ua:*bot* referer:~\"x'y\"");
    assert!(!query.sql.contains("DROP"));
    assert!(!query.sql.contains("x'y"));
    assert_eq!(query.params.len(), 3);
    assert!(query.sql.contains("u.raw_url = ?"));
    assert!(
        query
            .sql
            .contains("coalesce((a.raw LIKE ? ESCAPE '\\'), FALSE)")
    );
    assert!(
        query
            .sql
            .contains("coalesce((regexp_matches(t.referer, ?)), FALSE)")
    );
    assert!(query.sql.contains("LEFT JOIN user_agents a"));
}

#[test]
fn negation_keeps_rows_with_missing_values() {
    let query = compiled("-ua:*bot* group:status");
    assert_eq!(query.source, Source::DayUrlRollup);
    assert!(
        query
            .sql
            .contains("WHERE NOT coalesce((a.raw LIKE ? ESCAPE '\\'), FALSE)")
    );
}

#[test]
fn explicit_sort_and_tie_breakers() {
    let query = compiled("group:day,status sort:day:asc");
    assert_eq!(query.source, Source::HourRollup);
    assert!(
        query
            .sql
            .ends_with("ORDER BY 1 ASC NULLS LAST, 2 ASC NULLS LAST")
    );
}

#[test]
fn rejects_invalid_queries() {
    let query = parse("group:url sort:ip").unwrap();
    assert!(compile(&query, ctx()).is_err());
}

#[test]
fn pages_respect_the_query_limit() {
    let query = compiled("group:url limit:25");
    let page = |offset, size| query.sql_for(Some(Page { offset, size }));
    assert!(page(0, 10).ends_with(" LIMIT 10"));
    assert!(page(20, 10).ends_with(" LIMIT 5 OFFSET 20"));
    assert!(page(30, 10).ends_with(" LIMIT 0 OFFSET 30"));
    let unlimited = compiled("group:url");
    assert!(
        unlimited
            .sql_for(Some(Page {
                offset: 100,
                size: 50
            }))
            .ends_with(" LIMIT 50 OFFSET 100")
    );
    assert_eq!(unlimited.sql_for(None), unlimited.sql);
}

#[test]
fn bot_filters_stay_on_the_daily_rollup() {
    let query = compiled("bot:googlebot* verification:verified group:day,bot");
    assert_eq!(query.source, Source::DayUrlRollup);
    assert!(
        query
            .sql
            .contains("LEFT JOIN user_agents a ON a.ua_id = t.ua_id")
    );
    assert!(query.sql.contains("t.verification = ?"), "{}", query.sql);
    assert!(!query.sql.contains("bot_ips"));
}

#[test]
fn raw_verification_joins_verdicts_per_ip_and_bot() {
    let query = compiled("verification:spoofed group:ip");
    assert_eq!(query.source, Source::Hits);
    assert!(query.sql.contains(
        "LEFT JOIN user_agents a ON a.ua_id = t.ua_id \
         LEFT JOIN bot_ips v ON v.ip = t.ip AND v.bot = a.bot"
    ));
    let humans = compiled("-bot:* group:status");
    assert!(
        humans
            .sql
            .contains("NOT coalesce((a.bot LIKE ? ESCAPE '\\'), FALSE)"),
        "{}",
        humans.sql
    );
}
