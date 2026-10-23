# ADR 0082 — GEO-SEO Platform: rank checks on the standard queue, and local SEO

- Status: accepted
- Date: 2026-10-23

## Context
Rank tracking read every result page live, which DataForSEO prices about 70 % above its standard queue. Scheduled checks do not need an answer within seconds. Local businesses also asked for what Local Falcon and BrightLocal sell: where they show on Google Maps across their area, how they compare with the businesses around them, and their reviews.

## Decision
- **Scheduled rank checks go through the standard queue** (`octopuson-dataforseo/src/rank.rs`, `queue.rs`).
  - Tasks are posted in batches of up to 100 (`task_post`, priority 1, tag `keyword_id|tracker_id`). The run stays `running` while its tasks wait in `rank_tasks` (migration 16).
  - **The posting copy keeps the tasks.** They never sync, because only the copy that posted them can read them.
  - `collect_queued` reads answers once a minute, 500 at a time within 40 seconds. It backs off from 60 seconds to 15 minutes and gives up after 24 hours.
  - The run ends when every task is read, even after the program was closed.
  - **Price.** $0.0006 a page and $0.0006 for an AI Overview that loads later.
  - **"Check now" stays live.** On credits the queue is used only when the gateway forwards `task_post` and `task_get` for the account; until then credit checks are live.
  - The desktop collects every minute, even while the web runs the schedule. The server spawns a collect thread per due account.
- **Rank tracking views.**
  - Every place in one table (`rank_places`).
  - SERP feature ownership, kept as `own:<type>` inside the features list so synced rows keep their width.
  - Landing pages and competitors' positions per keyword.
  - A comparison with a chosen day, estimated visits (volume × CTR), and coordinates or postal codes as places.
- **Local SEO** (`octopuson-dataforseo/src/local.rs`, `modules/local.rs`, migration 17).
  - **The business.**
    - It is found by name and matched by cid, place id, website, then folded name.
    - Its profile (`my_business_info`: category, rating, hours, attributes) and the businesses around it (`business_listings/search`) are fetched.
  - **The grid.** 3–9 points a side and a 0.2–50 km radius. Each point asks `serp/google/maps` at its coordinates, depth 20.
    - Measures: ARP (average rank), ATRP (21 where it does not show) and SoLV (share of local voice, top-3 share).
    - The first three rivals at each point are kept.
    - **Price.** Live by hand ($0.002 a point); scheduled weekly or monthly through the queue ($0.0006 a point). A 7×7 grid costs about $0.03 a keyword a check.
  - **Reviews** come through the queue only ($0.00075 for ten, newest first). The rating is kept by day (`local_profiles`).
  - **Replies.** A reply draft comes from a template in four languages, or from an assistant on the user's own key (ADR 0084).
  - **The map.** The page draws the grid on MapLibre with OpenFreeMap tiles (the CSP allows `tiles.openfreemap.org`); squares when the map cannot load. Reports draw the grid as squares only.
  - **Sync.** Settings, runs with their points, reviews and profiles sync (`LATER_KINDS`); queue tasks do not.
- **Reports, snapshots and assistants.**
  - A `local` report part draws the grids, the measures, the rating and the rivals.
  - Snapshots keep the grid's measures per keyword and the rating (`local`, `SnapshotData` version 2). A comparison shows each map keyword's SoLV and ARP before and after and tells the change in words.
  - MCP `local_seo` is free.

## Consequences
- **Scheduled rank tracking costs about a third of before** on the user's own key. On credits the saving waits for the gateway's per-account task routes (P13).
- **Queued results arrive minutes later, not seconds.** Pages show a "queued" state and reload when the answers are read.
- **Local SEO gives agencies the grid view** at queue prices. Its accuracy depends on DataForSEO's Maps results at each coordinate.
- **New gateway routes are needed before local SEO works on credits:** `my_business_info`, `business_listings`, `serp/google/maps`, and the Maps and reviews tasks bound to the posting account. Until then, credits are refused with a clear message.
