# 0068: Explicit Schema and Rich Results preflight

## Decision

Add a separate **Schema & Rich Results** tool in Spider. It runs only after **Test**;
opening it, selecting a URL, changing render mode, or opening a saved crawl never
starts validation. URL Explorer's **Test Schema** action only prefills the form.
No changes are made to crawl configurations, archives, audit findings or scores.

Three sources are supported:

- URL: one explicit document, robots.txt and at most five HTTP redirects, using the
  existing list-mode crawler and an in-memory sink. Links, sitemaps and well-known
  files are not followed. Browser mode reuses the renderer and its request policies;
  browser subresources may load. The default is server HTML, regardless of the
  selected archive's rendering mode. Concurrent live tests are rejected.
- HTML: pasted source parsed locally, without script execution or network access.
- JSON-LD: pasted JSON parsed locally, without remote context retrieval.

Input and fetched DOM/body are bounded to 4 MiB. Up to 2,000 JSON-LD blocks and
2,000 object nodes are considered; a limit marks the report partial. An incomplete
HTTP request, redirect chain, truncated source, robots block or failed render has
no conclusive schema report. Raw fallback HTML is not a successful rendered test.
No DuckDB file or new crawl is created. Results last in the current tool view.

## Scope and honesty

Rule snapshot **2026-10-18.1** checks JSON syntax and selected JSON-LD properties
for Article/NewsArticle/BlogPosting, BreadcrumbList, Product snippets, Organization
and exact LocalBusiness. Product offers include Offer price (including zero and
priceSpecification) and AggregateOffer lowPrice/currency. Review/rating policy
and nested review requirements remain unverified. Other types, including supporting
entities, are retained as observations, not treated as invalid rich-result types.
Microdata and RDFa are detected but property validation is explicitly unverified.

Plain schema.org contexts and a single `@vocab` schema.org mapping are interpreted.
Other contexts, aliases and missing contexts are unverified. Advanced JSON-LD
`@set`, `@list`, `@reverse` and `@nest` expressions remain unverified. Nested objects,
arrays, `@graph`, `@included`, and same-document `@id` references across scripts
are considered. Conflicting or unresolved definitions are unverified; the tool
does not invent missing-field errors from them. Inactive template content is ignored.

Required and recommended properties are distinct. Article and Organization do
not get fabricated required-property errors. The last breadcrumb may omit `item`;
the item's Thing name can supply `name`. Product does not require offers if a
review/rating branch is provided. Offer currency is recommended for the product
snippet profile, whereas AggregateOffer currency is required. This is not a
merchant-listing validator.

Every finding displays the format, numbered block, JSON pointer, bounded observed
value, expectation, illustrative value when appropriate, and official guide link.
Item counts use mutually exclusive states: error, unverified, recommendation,
implemented checks passed. A passed subset is not a complete Schema.org validation
or Google eligibility verdict. The UI identifies remaining value, visible-content,
access and policy checks. A separate link opens the official Google Rich Results
Test; Spider does not call or impersonate that service. The web-only demo disables
execution instead of fabricating a result.

## Sources reviewed

- [General structured data policies](https://developers.google.com/search/docs/appearance/structured-data/sd-policies)
- [Article](https://developers.google.com/search/docs/appearance/structured-data/article)
- [Breadcrumb](https://developers.google.com/search/docs/appearance/structured-data/breadcrumb)
- [Product snippets](https://developers.google.com/search/docs/appearance/structured-data/product-snippet)
- [Organization](https://developers.google.com/search/docs/appearance/structured-data/organization)
- [LocalBusiness](https://developers.google.com/search/docs/appearance/structured-data/local-business)

## Verification

Parser tests cover context uncertainty, templates, nested/reference objects,
Article/Organization recommendations, last-breadcrumb exceptions, offer alternatives
and zero prices. Injected crawler tests check redirect-only traversal, DOM validation,
render failure and offline inputs. Desktop IPC tests check response shape and that
validation creates no saved crawl. Frontend tests check escaped evidence, source
limits, count semantics, partial outcomes and no automatic request on opening.
