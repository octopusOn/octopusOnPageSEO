# 0070 — Spider SEO opportunity analyzer

Date: 2026-10-18
Status: implemented; GSC connected in Google's test mode. Public distribution still requires publisher verification.

## Decision

Keep the analyzer inside Spider, independent of crawl scoring and archive migration. The pure `seonexus-opportunities` crate accepts projects, analysis requests and typed observations. `seonexus-spider-app::opportunities` owns desktop OAuth, imports, normalized storage, background jobs, pagination and report exports. The desktop IPC layer remains thin.

The UI is available under **SEO Fırsatları / SEO Opportunities**, even with no saved crawl. Multiple project configurations persist site, GSC/GA4 properties, brand terms, selected business goal and optional completed crawl evidence. Opening the page does not fetch Google reports, start a crawl or edit a website. Refresh and analysis are explicit, cancellable jobs.

## Observations and calculations

The UI defaults to the latest 28 days ending three days before the current Pacific date and the immediately preceding 28 days. GSC final web data is retrieved separately for pages, queries, query/page pairs and daily property totals. CTR is clicks / impressions and position is weighted by impressions. Query/page sums are labeled as a subset if no page report exists. Missing rows stay unknown, including previous URLs absent from the current export. Unequal periods do not trigger the loss rule.

* Low CTR: at least 1,000 impressions, position ≤20, observed CTR below 75% of the site's peer CTR. Peers share position bucket, country, device and brand classification. The candidate URL is excluded. At least 10 other URLs and 5,000 peer impressions are required; otherwise the result is an unquantified review candidate. Buckets: 1–3, >3–7, >7–10, >10–20. No external industry CTR table is used.
* Near win: query/page observations with position 4–20 and at least 200 impressions. The previous position bucket supplies a possible estimate when enough observations exist. A separate query export never invents a query/page relationship.
* Loss: an observed URL with ≥50 previous clicks, ≥10 fewer clicks and a decline of ≥20%. Impressions, CTR and position are shown separately. The rule does not diagnose content quality.

Signals share one URL row. Click impact is the maximum individual observed loss or estimated gain, never their sum. Priority = that maximum × confidence × page-level GA4 business contribution. When an estimate is unavailable, `ln(1 + page impressions)` is an explicitly labeled review order. Confidence multipliers are 1 observed, 0.65 limited and 0.35 review. With at least 100 sessions and comparable page measurements, business contribution is `1 + clamp(value / positive site median, 0, 1)`; value is selected event session rate or purchase revenue / sessions. Missing or ambiguous GA4 leaves ×1. Query conversions are never inferred.

## Google and coverage

Publisher setup uses a Google Cloud **Desktop app** OAuth client with Search Console API, Google Analytics Data API and Google Analytics Admin API enabled. Use **OAuth setup** in SEO Opportunities to import the Google Cloud Desktop client JSON. The validated client ID and secret are stored together in the OS credential store; the original JSON and its endpoints are not retained. Web clients, service-account keys and files over 64 KiB are rejected. Disconnect accounts and cancel pending sign-ins before changing the client. Builds may alternatively configure `OCTOPUSSEO_GOOGLE_CLIENT_ID` and `OCTOPUSSEO_GOOGLE_CLIENT_SECRET`; a locally imported client takes precedence. Client IDs are public app identifiers. Never commit personal credentials or refresh tokens. Prepare consent-screen branding, privacy policy and Google's required scope verification before distribution.

Authorization opens the system browser, uses PKCE S256 and a one-use cryptographic state, and binds only an ephemeral `127.0.0.1` callback port. Read-only scopes: `webmasters.readonly` and `analytics.readonly`. Refresh tokens live in macOS Keychain / Windows Credential Manager. Other targets use memory for the current session and disclose this limitation. Access tokens remain in memory. Cancellation, denied consent, expiry and redacted HTTP failures are visible. No configured client means sign-in is disabled; imports remain available.

The callback listener stays available for ten minutes so the user can review Google's test-app warning and consent. Status/job polling continues while the system browser is in front. Dialog focus is established on mount and does not reset during polling; discovered Google properties use bounded, accessible selectors.

GSC paginates in 25,000-row pages, GA4 in 10,000-row pages, bounded at 100,000 fetched rows per report. Bounded cancellable exponential retry handles quota/server responses; an interrupted GSC synchronization never commits partial reports. A GA4 failure can preserve fresh GSC, with stale GA4 explicitly excluded. GSC top-row/privacy limits are always disclosed. GA4 thresholding, sampling and aggregation loss are disclosed. Google organic means sessionSource `google` and sessionMedium `organic`. Selected event counts are queried with eventName filtering; session rates are not summed across events.

GSC uses Pacific dates. GA4 property timezone and currency are read from Admin metadata. Clicks and sessions remain distinct measurements. GA4 landing paths require explicit confirmation that the property belongs to the project's origin; ambiguous multi-domain mapping does not boost priority. Country/device/brand-filtered GSC does not silently reuse unsegmented GA4 totals. HTTP scheme/host, language paths, case and meaningful query parameters are retained; fragments alone are removed.

## Imports and storage

CSV/TSV (UTF-8/BOM, UTF-16, comma/semicolon/tab, quoting) and XLSX cached-cell values are supported. Preview shows project scope, source, grain, date range, sheet, column mapping, counts and sampled records. TR/EN number parsing is explicit; numeric XLSX cells keep their native decimal semantics. Excel formulas are never evaluated. Files are capped at 25 MiB / 100,000 rows / 128 columns; expanded OOXML is bounded, external relationships and document entities are rejected. Legacy XLS is not supported; export as XLSX or CSV. Dates accept ISO, compact GSC dates or native Excel date serials (1900 system).

A preview digest binds file bytes, effective mapping, dates, number format, source, grain, GA4 scope confirmation and goal. Changed input requires a fresh preview. Invalid rows prevent commit. Reimport replaces the exact project/source/grain/period slot, rather than adding duplicates. Different projects stay isolated.

All module data is stored in one atomically replaced `opportunities/opportunities.json.zst`, beside the crawl directory. No new DuckDB archive or raw API response is retained. The expanded/compressed bounds are 128/64 MiB, and the UI shows actual disk usage. Only the oldest re-fetchable API datasets are pruned after 12 per project; imports are retained until project removal. Property/site bindings prevent relabeled old datasets from being analyzed under a changed project. Clearing API cache preserves imports. Crawl evidence reads only an existing, compatible, completed archive and displays its date.

## Verification

Automated tests cover observed loss/unknown absence, weighted metrics, CTR peers, overlapping signals, origin/query identity, GA4 neutrality and stale data, import formats and missing columns, XLSX XML escaping, transactional replacement, OAuth denial/cancel/expiry, quota, interrupted transports, pagination and exports. Desktop IPC and existing Spider regressions run alongside frontend tests and production compilation.

Before production Google availability, verify one selected URL against actual GSC and GA4 source reports for the same periods, properties, organic scope and timezone. This cannot be replaced by mocked API tests. Initial release produces recommendations only.

References: [Desktop OAuth](https://developers.google.com/identity/protocols/oauth2/native-app), [GSC query](https://developers.google.com/webmaster-tools/v1/searchanalytics/query), [GA4 schema](https://developers.google.com/analytics/devguides/reporting/data/v1/api-schema), [GA4 runReport](https://developers.google.com/analytics/devguides/reporting/data/v1/rest/v1beta/properties/runReport).


## Follow-up — 2026-10-18.2

Incomplete position coverage no longer produces an average over only the known impressions. GA4 rates, event counts and revenue require complete measurements for the aggregated URL; missing values remain unknown. Thresholding, row caps or interrupted pagination do not increase business-value priority. A truncated report does not supply a whole-property total.

Google refresh reports phase, period, received rows, request count and retry waits for every fetched page. GA4 pagination follows `rowCount`, uses a stable landing-page order and marks an early empty page incomplete. See [GA4 pagination](https://developers.google.com/analytics/devguides/reporting/data/v1/basics#pagination) and [RunReportResponse](https://developers.google.com/analytics/devguides/reporting/data/v1/rest/v1beta/RunReportResponse). Cancelling during encoding is checked before the atomic store replacement. Sync and analysis share the two-operation limit; bounded job history retains the latest operation for other projects.

Import previews reject query/page reports mislabeled as page totals and percent values mapped to counts. Rate columns accept Turkish prefix or English suffix percentage notation. Dedicated regressions cover partial position, mixed missing GA4 measurements, short/empty API pages, progress, cancellation before persistence and cross-project job history.

## Follow-up — 2026-10-18.3

Live GSC validation exposed a difference between property totals and detailed row sums. Page reports now request only the page dimension with automatic aggregation, avoiding unnecessary country/device breakdown. Query/page reports retain those segments for peer comparisons. For an unfiltered matching property scope, overview metrics use the daily property report, with impression-weighted position and recomputed CTR. Filtered or narrower scopes use observed page/subset totals and explicitly say so. Missing or truncated daily totals never become a full-property total. Segmented API query subsets do not assert confirmed page traffic losses.

See Google's [data extraction guidance](https://developers.google.com/webmaster-tools/v1/how-tos/all-your-data): grouping by page or query may drop rows even below the API row limit. Accurate property totals and URL observations are distinct evidence; missing URL/query rows remain unknown.
