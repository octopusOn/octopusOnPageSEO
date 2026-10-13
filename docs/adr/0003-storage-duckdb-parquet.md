# ADR 0003 — Storage: catalog + per-site DuckDB rollups + monthly Parquet partitions

- Status: accepted (revised 2026-10-12 after the M2 implementation)
- Date: 2026-10-12

## Context
Sites may accumulate billions of log lines (100 GB+ raw). Users append new logs over time and expect dashboards and filter templates to respond in well under a second.

## Decision

### Layout
```text
<data>/catalog.duckdb                       clients, sites (format spec per site)
<data>/sites/<id>/site.duckdb               dimensions, rollups, import history
<data>/sites/<id>/.lock                     single-writer lock (std File::try_lock)
<data>/sites/<id>/hits/month=YYYY-MM/       immutable ZSTD Parquet hit files
<data>/sites/<id>/hits/.staging-<import>/   files of an import not yet published
```

### Hit files
- Columns: `ts, ip, method, url_id, protocol, status, bytes, referer, ua_id, duration_us, host, router, upstream, request_seq, line_hash` (month is the Hive partition key).
- URLs and user agents are replaced by ids from the `urls` / `user_agents` dimension tables (high repetition, needed by rollups and URL rules). Referer, host, router and upstream stay strings: Parquet dictionary encoding already compresses them well and they are rarely grouped.
- Measured: 519 MB of Traefik CLF → 44 MB Parquet (≈ 8.5 %).

### Rollups
- `r_day_url (day, url_id, ua_id, status, hits, bytes, duration_sum/count/max, first_ts, last_ts, import_id)` and `r_hour (hour, status, hits, bytes, import_id)`.
- Insert-only partial aggregates per flush; queries `SUM` them. Keyed by `ua_id` so bot classification (M4) can be applied at query time without rewriting rollups.

### Import transaction
- One import per input file, in one DuckDB transaction. Parsed hits are built as Arrow batches on worker threads and appended into a `TEMP staging_hits` table.
- Every `flush_rows` rows (default 4 M): de-duplicate → register new URLs/UAs (anti-join) → `COPY (staging ⋈ dimensions) TO` the staging directory (`PARTITION_BY month`, ids resolved while streaming, no intermediate table) → insert rollups (aggregated on raw strings first, then only the aggregate is joined to the dimensions).
- Measured (14-core laptop, release): 10 M lines in 14.5 s (≈ 690 k lines/s); 100 M lines in ≈ 3.5 min. The sequential flush (append, Parquet encode, rollups) is the bottleneck, not parsing; flush size between 0.5 M and 4 M rows makes no measurable difference.
- Commit: write `imports`, `imported_files`, `import_skips`, `import_samples`, `COMMIT`, then rename staged files into `hits/month=*/`.
- Dropping a session (error or cancellation) rolls back and deletes the staging directory.
- Recovery on site open: staging directories of committed imports are published (renames are idempotent); others are deleted.

### De-duplication
1. File fingerprint: BLAKE3 over size + first/last 64 KiB → already imported files are skipped without reading.
2. Hour overlap: only hours present in `r_hour` of *other* imports are examined.
3. Count-aware multiset match on `xxh3_64(raw line)` against those hours' Parquet rows: a line is dropped only as many times as earlier imports contain it (identical lines in the same second are legitimate). A per-session `dedupe_matched` table keeps this correct across flushes. Lines are never de-duplicated within the same import.

### Resource limits
- DuckDB `memory_limit` defaults to 1 GiB (`--memory-limit`, `OCTOPUSSEO_MEMORY_LIMIT_MB`); larger work spills to disk. DuckDB's default (80 % of RAM) is too greedy for a desktop app.
- Known gap: process RSS is ≈ 2 GB at the 1 GiB limit (up to 3.6 GB on a 100 M-line de-duplication pass). The limit governs DuckDB's buffer pool only; in-flight Arrow batches, parse chunks and allocator retention come on top. To be profiled before the desktop release.
- Extension autoload/autoinstall is disabled; Parquet is compiled in (`bundled` + `parquet`), so nothing is downloaded at runtime.
- `urls.raw_url` uniqueness is guaranteed by the importer under the site lock rather than an ART index, which would cost memory on millions of URLs.

## Consequences
- Most templates are answered from rollups in time independent of total rows.
- Old months can be dropped by deleting files plus their rollup rows.
- DuckDB allows a single writer process; writes take a per-site lock and report "site is busy".
- Rollup schemas are versioned (forward-only migrations) and rebuildable from Parquet.
- Deferred: multi-domain split by host/router, retention policy, Parquet compaction of small files.
