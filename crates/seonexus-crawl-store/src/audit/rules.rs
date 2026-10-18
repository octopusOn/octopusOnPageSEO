//! SQL predicates for checks on individual URLs and their stored relationships.

/// Each query returns one `url` per affected page. Rule ids belong to the shared catalog.
pub(super) const RULES: &[(&str, &str)] = &[
    (
        "content-near-duplicate",
        "SELECT u.url FROM content_matches m JOIN urls u ON u.id=m.page",
    ),
    (
        "render-error",
        "SELECT u.url FROM rendering r JOIN urls u ON u.id=r.page WHERE NOT r.rendered AND r.error NOT LIKE '%cancelled%'",
    ),
    (
        "render-truncated",
        "SELECT u.url FROM rendering r JOIN urls u ON u.id=r.page WHERE r.truncated",
    ),
    (
        "render-noindex-removed",
        "SELECT u.url FROM rendering r JOIN audit_pages u ON u.id=r.page WHERE r.rendered AND NOT r.truncated AND r.raw_noindex AND NOT u.noindex",
    ),
    (
        "llms-txt-missing",
        "SELECT url FROM site_files WHERE status IN (404,410)",
    ),
    (
        "sitemap-missing",
        "SELECT url FROM sitemaps s WHERE NOT EXISTS (SELECT 1 FROM sitemaps ok WHERE ok.status=200 AND ok.error IS NULL AND regexp_extract(ok.url,'^https?://[^/]+')=regexp_extract(s.url,'^https?://[^/]+'))",
    ),
    (
        "sitemap-not-in-robots",
        "SELECT s.url FROM sitemaps s WHERE s.status=200 AND s.error IS NULL AND EXISTS (SELECT 1 FROM robots_files r WHERE regexp_extract(r.url,'^https?://[^/]+')=regexp_extract(s.url,'^https?://[^/]+') AND r.http_status=200) AND NOT EXISTS (SELECT 1 FROM robots_files r WHERE contains(r.sitemaps,s.url))",
    ),
    (
        "internal-blocked-robots",
        "SELECT url FROM urls WHERE internal AND blocked_by_robots",
    ),
    (
        "blocked-resource",
        "SELECT t.url FROM urls t JOIN links l ON l.target=t.id WHERE t.blocked_by_robots AND l.kind IN ('image','stylesheet','script','iframe','media')",
    ),
    (
        "internal-no-response",
        "SELECT url FROM urls WHERE internal AND error IS NOT NULL",
    ),
    (
        "internal-redirect",
        "SELECT url FROM urls WHERE internal AND status IN (301,302,303,307,308)",
    ),
    (
        "internal-redirect-temporary",
        "SELECT url FROM urls WHERE internal AND status IN (302,303,307)",
    ),
    (
        "internal-client-error",
        "SELECT url FROM urls WHERE internal AND status BETWEEN 400 AND 499",
    ),
    (
        "internal-server-error",
        "SELECT url FROM urls WHERE internal AND status BETWEEN 500 AND 599",
    ),
    (
        "external-no-response",
        "SELECT url FROM urls WHERE NOT internal AND error IS NOT NULL",
    ),
    (
        "external-client-error",
        "SELECT url FROM urls WHERE NOT internal AND status BETWEEN 400 AND 499",
    ),
    (
        "external-server-error",
        "SELECT url FROM urls WHERE NOT internal AND status BETWEEN 500 AND 599",
    ),
    (
        "redirect-chain",
        "SELECT s.url FROM urls s JOIN urls t ON s.redirect_url=t.url WHERE s.status IN (301,302,303,307,308) AND t.status IN (301,302,303,307,308)",
    ),
    (
        "meta-refresh",
        "SELECT url FROM urls WHERE meta_refresh_url IS NOT NULL",
    ),
    (
        "links-to-redirects",
        "SELECT s.url FROM links l JOIN urls s ON s.id=l.source JOIN urls t ON t.id=l.target WHERE l.kind='hyperlink' AND t.status IN (301,302,303,307,308)",
    ),
    (
        "links-to-broken",
        "SELECT s.url FROM links l JOIN urls s ON s.id=l.source JOIN urls t ON t.id=l.target WHERE l.kind='hyperlink' AND t.internal AND (t.status>=400 OR t.error IS NOT NULL)",
    ),
    (
        "links-to-broken-external",
        "SELECT s.url FROM links l JOIN urls s ON s.id=l.source JOIN urls t ON t.id=l.target WHERE l.kind='hyperlink' AND NOT t.internal AND (t.status>=400 OR t.error IS NOT NULL)",
    ),
    (
        "http-url",
        "SELECT url FROM urls WHERE internal AND starts_with(url,'http://')",
    ),
    (
        "http-links",
        "SELECT s.url FROM links l JOIN urls s ON s.id=l.source WHERE l.kind='hyperlink' AND starts_with(s.url,'https://') AND starts_with(l.target_url,'http://')",
    ),
    (
        "mixed-content",
        "SELECT url FROM urls WHERE mixed_content>0",
    ),
    (
        "insecure-form",
        "SELECT url FROM urls WHERE insecure_forms>0",
    ),
    (
        "missing-hsts",
        "SELECT url FROM audit_pages WHERE starts_with(url,'https://') AND NOT hsts",
    ),
    (
        "url-uppercase",
        "SELECT url FROM urls WHERE internal AND regexp_matches(regexp_replace(url,'^https?://[^/]+',''),'[A-Z]')",
    ),
    (
        "url-underscores",
        "SELECT url FROM urls WHERE internal AND contains(regexp_replace(url,'^https?://[^/]+',''),'_')",
    ),
    (
        "url-non-ascii",
        "SELECT url FROM urls WHERE internal AND regexp_matches(url,'[^\\x00-\\x7F]')",
    ),
    (
        "url-multiple-slashes",
        "SELECT url FROM urls WHERE internal AND contains(regexp_replace(url,'^https?://',''),'//')",
    ),
    (
        "url-spaces",
        "SELECT url FROM urls WHERE internal AND (contains(url,'%20') OR contains(url,' '))",
    ),
    (
        "url-parameters",
        "SELECT url FROM urls WHERE internal AND contains(url,'?')",
    ),
    (
        "url-too-long",
        "SELECT url FROM urls WHERE internal AND length(url)>115",
    ),
    (
        "title-missing",
        "SELECT url FROM audit_indexable WHERE coalesce(trim(title),'')=''",
    ),
    (
        "title-multiple",
        "SELECT url FROM audit_pages WHERE title_count>1",
    ),
    (
        "title-duplicate",
        "SELECT url FROM audit_indexable WHERE trim(title)<>'' QUALIFY count(*) OVER (PARTITION BY title)>1",
    ),
    (
        "title-too-long",
        "SELECT url FROM audit_pages WHERE length(title)>60",
    ),
    (
        "title-too-short",
        "SELECT url FROM audit_pages WHERE length(trim(title)) BETWEEN 1 AND 29",
    ),
    (
        "title-same-as-h1",
        "SELECT url FROM audit_pages WHERE trim(title)<>'' AND lower(trim(title))=lower(trim(h1))",
    ),
    (
        "description-missing",
        "SELECT url FROM audit_indexable WHERE coalesce(trim(meta_description),'')=''",
    ),
    (
        "description-multiple",
        "SELECT url FROM audit_pages WHERE meta_description_count>1",
    ),
    (
        "description-duplicate",
        "SELECT url FROM audit_indexable WHERE trim(meta_description)<>'' QUALIFY count(*) OVER (PARTITION BY meta_description)>1",
    ),
    (
        "description-too-long",
        "SELECT url FROM audit_pages WHERE length(meta_description)>155",
    ),
    (
        "description-too-short",
        "SELECT url FROM audit_pages WHERE length(trim(meta_description)) BETWEEN 1 AND 69",
    ),
    (
        "h1-missing",
        "SELECT url FROM audit_indexable WHERE h1_count=0",
    ),
    (
        "h1-empty",
        "SELECT url FROM audit_pages WHERE h1_count>0 AND coalesce(trim(h1),'')=''",
    ),
    (
        "h1-multiple",
        "SELECT url FROM audit_pages WHERE h1_count>1",
    ),
    (
        "h1-duplicate",
        "SELECT url FROM audit_indexable WHERE trim(h1)<>'' QUALIFY count(*) OVER (PARTITION BY h1)>1",
    ),
    (
        "h1-too-long",
        "SELECT url FROM audit_pages WHERE length(h1)>70",
    ),
    (
        "h2-missing",
        "SELECT url FROM audit_indexable WHERE h2_count=0",
    ),
    (
        "h2-duplicate",
        "SELECT url FROM audit_indexable WHERE trim(h2)<>'' QUALIFY count(*) OVER (PARTITION BY h2)>1",
    ),
    (
        "headings-non-sequential",
        "SELECT u.url FROM (SELECT page,level,lag(level,1,0) OVER(PARTITION BY page ORDER BY position) AS previous FROM headings) h JOIN audit_pages u ON u.id=h.page WHERE h.level>h.previous+1",
    ),
    (
        "content-empty",
        "SELECT url FROM audit_pages WHERE word_count=0",
    ),
    (
        "content-low",
        "SELECT url FROM audit_indexable WHERE indexability='indexable' AND word_count BETWEEN 1 AND 199",
    ),
    (
        "content-exact-duplicate",
        "SELECT url FROM audit_indexable WHERE word_count>0 AND text_hash IS NOT NULL QUALIFY count(*) OVER (PARTITION BY text_hash)>1",
    ),
    (
        "soft-404",
        "SELECT url FROM audit_pages WHERE word_count<200 AND regexp_matches(lower(coalesce(title,'') || ' ' || coalesce(h1,'')), '(404|page not found|sayfa bulunamadı|not found)')",
    ),
    (
        "image-broken",
        "SELECT p.url FROM images i JOIN urls p ON p.id=i.page JOIN urls t ON t.url=i.src WHERE t.status>=400 OR t.error IS NOT NULL",
    ),
    (
        "image-missing-alt",
        "SELECT p.url FROM images i JOIN urls p ON p.id=i.page WHERE i.alt IS NULL",
    ),
    (
        "image-alt-too-long",
        "SELECT p.url FROM images i JOIN urls p ON p.id=i.page WHERE length(i.alt)>100",
    ),
    (
        "image-too-large",
        "SELECT p.url FROM images i JOIN urls p ON p.id=i.page JOIN urls t ON t.url=i.src WHERE t.size_bytes>102400",
    ),
    (
        "image-missing-dimensions",
        "SELECT p.url FROM images i JOIN urls p ON p.id=i.page WHERE NOT i.has_dimensions",
    ),
    (
        "canonical-missing",
        "SELECT url FROM audit_indexable WHERE canonical IS NULL AND header_canonical IS NULL",
    ),
    (
        "canonicalised",
        "SELECT url FROM audit_pages WHERE indexability='canonicalised'",
    ),
    (
        "canonical-multiple",
        "SELECT url FROM audit_pages WHERE canonical_count>1",
    ),
    (
        "canonical-mismatch",
        "SELECT url FROM audit_pages WHERE canonical IS NOT NULL AND header_canonical IS NOT NULL AND canonical<>header_canonical",
    ),
    (
        "canonical-to-non-200",
        "SELECT p.url FROM audit_pages p JOIN urls t ON t.url=coalesce(p.canonical,p.header_canonical) WHERE t.status<>200 OR t.error IS NOT NULL",
    ),
    (
        "canonical-to-non-indexable",
        "SELECT p.url FROM audit_pages p JOIN urls t ON t.url=coalesce(p.canonical,p.header_canonical) WHERE t.indexability<>'indexable'",
    ),
    (
        "canonical-cross-domain",
        "SELECT url FROM audit_pages WHERE regexp_extract(url,'^https?://([^/:]+)',1)<>regexp_extract(coalesce(canonical,header_canonical),'^https?://([^/:]+)',1)",
    ),
    (
        "canonical-to-http",
        "SELECT url FROM audit_pages WHERE starts_with(url,'https://') AND starts_with(coalesce(canonical,header_canonical),'http://')",
    ),
    (
        "canonical-with-noindex",
        "SELECT url FROM audit_pages WHERE noindex AND coalesce(canonical,header_canonical)<>url",
    ),
    ("noindex", "SELECT url FROM audit_pages WHERE noindex"),
    (
        "nofollow-page",
        "SELECT url FROM audit_pages WHERE nofollow",
    ),
    (
        "snippet-restricted",
        "SELECT url FROM audit_pages WHERE nosnippet OR max_snippet=0",
    ),
    (
        "lang-missing",
        "SELECT url FROM audit_pages WHERE coalesce(trim(lang),'')=''",
    ),
    (
        "hreflang-missing-return",
        "SELECT p.url FROM hreflang h JOIN urls p ON p.id=h.page JOIN audit_hreflang_pages t ON t.url=h.target WHERE p.internal AND h.target<>p.url AND NOT EXISTS (SELECT 1 FROM hreflang back WHERE back.page=t.id AND back.target=p.url)",
    ),
    (
        "hreflang-to-non-200",
        "SELECT p.url FROM hreflang h JOIN urls p ON p.id=h.page JOIN urls t ON t.url=h.target WHERE p.internal AND (t.status<>200 OR t.error IS NOT NULL)",
    ),
    (
        "hreflang-to-non-indexable",
        "SELECT p.url FROM hreflang h JOIN urls p ON p.id=h.page JOIN urls t ON t.url=h.target WHERE p.internal AND t.indexability<>'indexable'",
    ),
    (
        "hreflang-missing-self",
        "SELECT p.url FROM audit_hreflang_pages p WHERE EXISTS (SELECT 1 FROM hreflang h WHERE h.page=p.id) AND NOT EXISTS (SELECT 1 FROM hreflang h WHERE h.page=p.id AND h.target=p.url)",
    ),
    (
        "hreflang-missing-x-default",
        "SELECT p.url FROM audit_hreflang_pages p WHERE EXISTS (SELECT 1 FROM hreflang h WHERE h.page=p.id) AND NOT EXISTS (SELECT 1 FROM hreflang h WHERE h.page=p.id AND lower(h.lang)='x-default')",
    ),
    (
        "hreflang-lang-mismatch",
        "SELECT p.url FROM hreflang h JOIN urls p ON p.id=h.page WHERE p.internal AND h.target=p.url AND lower(h.lang)<>'x-default' AND split_part(lower(h.lang),'-',1)<>split_part(lower(p.lang),'-',1)",
    ),
    (
        "pagination-to-non-200",
        "SELECT p.url FROM audit_pages p JOIN urls t ON t.url=p.rel_next OR t.url=p.rel_prev WHERE t.status<>200 OR t.error IS NOT NULL",
    ),
    (
        "pagination-canonicalised",
        "SELECT url FROM audit_pages WHERE (rel_next IS NOT NULL OR rel_prev IS NOT NULL) AND indexability='canonicalised'",
    ),
    (
        "links-dead-end",
        "SELECT p.url FROM audit_indexable p JOIN link_counts c ON p.id=c.id WHERE c.outlinks=0",
    ),
    (
        "links-too-many",
        "SELECT p.url FROM audit_pages p JOIN link_counts c ON p.id=c.id WHERE c.outlinks>300",
    ),
    (
        "links-internal-nofollow",
        "SELECT p.url FROM links l JOIN urls p ON p.id=l.source WHERE l.kind='hyperlink' AND l.internal AND l.nofollow",
    ),
    (
        "anchor-empty",
        "SELECT p.url FROM named_links l JOIN urls p ON p.id=l.source WHERE l.kind='hyperlink' AND trim(l.text)=''",
    ),
    (
        "orphan-page",
        "SELECT url FROM audit_pages WHERE depth IS NULL AND EXISTS (SELECT 1 FROM sitemap_urls s WHERE coalesce(s.crawl_url,s.url)=audit_pages.url) AND NOT EXISTS (SELECT 1 FROM links l WHERE l.target=audit_pages.id AND l.kind='hyperlink')",
    ),
    (
        "single-inlink",
        "SELECT p.url FROM audit_indexable p JOIN link_counts c ON p.id=c.id WHERE c.unique_inlinks=1 AND p.depth>0",
    ),
    (
        "deep-page",
        "SELECT url FROM audit_indexable WHERE depth>=4",
    ),
    (
        "sitemap-error",
        "SELECT url FROM sitemaps WHERE error IS NOT NULL OR status>=400",
    ),
    (
        "sitemap-too-large",
        "SELECT url FROM sitemaps WHERE entries>50000",
    ),
    (
        "sitemap-non-indexable",
        "SELECT u.url FROM urls u JOIN sitemap_urls s ON coalesce(s.crawl_url,s.url)=u.url WHERE u.indexability<>'indexable'",
    ),
    (
        "sitemap-missing-indexable",
        "SELECT url FROM audit_indexable WHERE indexability='indexable' AND EXISTS (SELECT 1 FROM sitemaps WHERE error IS NULL AND status=200) AND NOT EXISTS (SELECT 1 FROM sitemap_urls s WHERE coalesce(s.crawl_url,s.url)=audit_indexable.url)",
    ),
    (
        "structured-data-invalid",
        "SELECT p.url FROM structured_data s JOIN urls p ON p.id=s.page WHERE NOT s.valid",
    ),
    (
        "structured-data-missing",
        "SELECT url FROM audit_indexable WHERE NOT EXISTS (SELECT 1 FROM structured_data s WHERE s.page=audit_indexable.id)",
    ),
    (
        "schema-required-invalid",
        "SELECT u.url FROM saved_schema_checks s JOIN urls u ON u.id=s.page WHERE u.internal AND NOT s.limited AND s.required_errors>0",
    ),
    (
        "schema-recommended",
        "SELECT u.url FROM saved_schema_checks s JOIN urls u ON u.id=s.page WHERE u.internal AND NOT s.limited AND s.warnings>0",
    ),
    (
        "schema-unverified",
        "SELECT u.url FROM saved_schema_checks s JOIN urls u ON u.id=s.page WHERE u.internal AND (s.limited OR s.error IS NOT NULL OR s.unchecked_items>0)",
    ),
    (
        "slow-response",
        "SELECT url FROM audit_pages WHERE response_ms>1000",
    ),
    (
        "large-html",
        "SELECT url FROM audit_pages WHERE size_bytes>1048576",
    ),
    (
        "uncompressed",
        "SELECT url FROM audit_pages WHERE size_bytes>=1024 AND coalesce(content_encoding,'identity') IN ('','identity')",
    ),
    ("truncated", "SELECT url FROM urls WHERE truncated"),
    (
        "viewport-missing",
        "SELECT url FROM audit_pages WHERE viewport IS NULL",
    ),
    (
        "viewport-not-responsive",
        "SELECT url FROM audit_pages WHERE viewport IS NOT NULL AND NOT contains(lower(viewport),'device-width')",
    ),
    (
        "open-graph-missing",
        "SELECT url FROM audit_indexable WHERE og_title IS NULL OR og_description IS NULL OR og_image IS NULL",
    ),
    (
        "twitter-card-missing",
        "SELECT url FROM audit_indexable WHERE twitter_card IS NULL",
    ),
    (
        "robots-txt-missing",
        "SELECT url FROM robots_files WHERE http_status IN (404,410)",
    ),
    (
        "robots-txt-unreachable",
        "SELECT url FROM robots_files WHERE http_status>=500 OR status='unreachable'",
    ),
];

#[cfg(test)]
mod navigation_tests {
    use super::RULES;
    use duckdb::Connection;

    #[test]
    fn navigation_rules_do_not_turn_non_navigation_3xx_into_redirect_findings() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE urls(id INTEGER,url VARCHAR,internal BOOLEAN,status USMALLINT,redirect_url VARCHAR);
            CREATE TABLE links(source INTEGER,target INTEGER,kind VARCHAR);
            INSERT INTO urls VALUES
              (0,'root',true,200,NULL),(1,'real',true,301,'temporary'),
              (2,'temporary',true,302,'end'),(3,'not-modified',true,304,'temporary'),
              (4,'choice',true,300,'temporary'),(5,'unused',true,306,'temporary'),
              (6,'to-not-modified',true,308,'not-modified'),(7,'end',true,200,NULL),
              (8,'external',false,301,'end');
            INSERT INTO links VALUES (0,3,'hyperlink'),(0,4,'hyperlink'),(0,5,'hyperlink'),
              (7,1,'hyperlink'),(3,2,'canonical');").unwrap();
        for (rule, expected) in [
            (
                "internal-redirect",
                vec!["real", "temporary", "to-not-modified"],
            ),
            ("internal-redirect-temporary", vec!["temporary"]),
            ("redirect-chain", vec!["real"]),
            ("links-to-redirects", vec!["end"]),
        ] {
            let sql = RULES.iter().find(|(id, _)| *id == rule).unwrap().1;
            let mut urls = connection
                .prepare(sql)
                .unwrap()
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            urls.sort();
            assert_eq!(urls, expected, "{rule}");
        }
    }
}
