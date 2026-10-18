# 0072 — WebPage and FAQPage local property checks

Date: 2026-10-18
Status: implemented

## Decision

Rule snapshot `2026-10-18.2` adds local Schema.org WebPage and FAQPage checks to the shared validator used by explicit tests and opted-in crawls. These are structural checks, not Google rich-result eligibility verdicts.

WebPage has no invented mandatory checklist. Only supplied name/description/date text shapes, URL values (including relative URLs), language values, breadcrumb and related-entity representations are checked. Omitted optional properties, textual breadcrumbs, multiple values, JSON-LD value objects and external entity identifiers do not create missing-property warnings. Date text is not a complete ISO-date or chronology check. Unrecognized page properties remain outside this profile.

FAQPage checks mainEntity question objects, nonempty question names, acceptedAnswer or suggestedAnswer objects and readable answer text. A single object and multiple questions/answers are accepted; basic HTML in answer text is accepted, while empty HTML does not provide an answer. Property warnings include the parent block, resolved property path, current value, expected shape and available illustrative corrections. Unknown/conflicting references, advanced contexts and unsupported answer alternatives remain unverified rather than being inferred as missing fields. No remote context or referenced entity is fetched.

These page/FAQ property observations are advisory warnings, never `schema-required-invalid` deductions. JSON syntax failures and existing supported Google required-property checks retain their scoring behavior. Google [retired FAQ rich results on May 7, 2026](https://developers.google.com/search/updates#may-2026); the retired feature's restrictions must not become score penalties or repeated warnings. Vocabulary references: [WebPage](https://schema.org/WebPage), [FAQPage](https://schema.org/FAQPage), [Question](https://schema.org/Question), [Answer](https://schema.org/Answer).

## Components and saved evidence

ListItem, Question and Answer objects consumed by a parent profile are represented by that parent's checks and paths, instead of separate unsupported-feature observations. Referenced definitions across blocks are matched by resolved content; unconsumed components, unrelated types, foreign contexts and conflicting definitions are never silently marked checked. Existing required-property profiles take precedence over page markers in multi-typed objects.

Stored crawl reports retain their recorded rule version and score. Opening an older report does not rerun validation or rewrite an archive. The UI labels older rule snapshots and offers the existing explicit retest/new crawl workflows. No migration is needed. A new opted-in crawl is required to update site-wide archived findings; an explicit URL test never changes an archived score.

Tests cover minimal pages, optional/value-object/array forms, inline and referenced questions, cross-block definitions, blank text/HTML, advisory-only scoring, unsupported and conflicting references, component suppression and saved report presentation.
