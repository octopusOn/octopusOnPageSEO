# 0071 — Optional crawl Schema validation and scoring

Date: 2026-10-18
Status: implemented

## Decision

Add `CrawlConfig.schema_validation`, default false, to Configuration and the new/repeat crawl dialog. The existing standalone Schema tool remains an explicit single-document test. The crawl option reuses that local validator against the already fetched HTML or successful JavaScript DOM, without another request or stored HTML requirement.

Supported required-property failures create `schema-required-invalid` findings. Syntax failures use the existing `structured-data-invalid` rule, with no second required-property deduction for the same syntax failure. Missing markup, recommended properties and unsupported/partial checks carry no penalty in this mode. This is an application health score, not a Google ranking or eligibility verdict. See [Google's structured-data guidelines](https://developers.google.com/search/docs/appearance/structured-data/sd-policies).

Schema-enabled crawls use score model 5 and expose inspected, scored, erroneous, recommended, missing and unverified URL coverage in SEO Röntgeni. The existing per-URL cap and proportional attribution remain in use. Each required-field rule costs 20 points on its affected URL before that cap; overall impact is averaged over the scored internal sample. Coverage and point loss are separate measurements. Checked types with recommendations remain eligible observations; unsupported types do not become confirmed successes.

## Evidence and compatibility

Schema 11 adds `schema_checks` only to new/upgraded databases. One compact report per checked page retains item type, JSON pointer, observed value, expected value, example and documentation. Input is limited to 4 MiB and saved reports to 256 KiB per page; source/DOM truncation, failed rendering, processing and storage limits stay unverified. No whole response HTML is retained for this feature.

The URL inspector and score remediation use saved reports; opening details never starts validation or a request. Compatible older archives receive an empty temporary evidence view and retain score model 4; they do not migrate on read just to enable this feature. They require a new opted-in crawl for property evidence. Existing syntax observations remain available. Comparison absence is unknown unless the counterpart has a complete supported Schema observation.

Tests exercise enabled/disabled crawls, required fields, malformed JSON, optional recommendations, absent/unsupported markup, JavaScript DOM changes and failed/truncated rendering, saved remediation evidence and version-5 frontend rendering.
