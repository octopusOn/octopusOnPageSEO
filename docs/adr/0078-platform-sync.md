# ADR 0078 — GEO-SEO Platform: desktops sync with the account's web workspace

- Status: accepted
- Date: 2026-10-22

## Context
GEO-SEO Platform 0.1.0 shipped as a desktop program and as app.octopuson.com (ADR 0077), each with its own workspace. A person signed in to both with the same account saw their desktop projects missing on the web. The owner asked for continuous, automatic sync through the account.

Decisions taken with the owner:
- **Projects sync on their own** between the desktop and the web, through the OctopusOn SEO account.
- **Scheduled work runs on the web server while a desktop syncs.** Nothing is paid twice, and schedules keep running while the computer is off. With sync switched off, the desktop runs its own schedules as before. Work started by hand runs wherever it was started.
- **Own API keys never travel.** They stay on each device, as do the device's settings (payment mode, budgets, proxy) and its spending records.

## Decision
- **The web workspace is the hub; each desktop is a copy.**
  - The desktop sends `POST /api/sync` with the account token every OctopusOn SEO program shares (`Authorization: Bearer oct_…`).
  - The server asks the gateway whose token it is (`/v1/me`), remembers the answer for five minutes, and answers from that account's workspace.
  - All of the logic is in the core:
    - `octopuson-platform::sync` keeps records in step;
    - `octopuson-platform-app::sync` has the hub's answer and the copy's round.
  - Two workspaces therefore sync in a test without a network.
- **Records travel by uid, never by local id.** Every travelling record has a `uid` that is the same on every copy:
  - **Random** for things made apart (projects, lists, reports, runs, audits).
  - **Drawn from what makes it unique** where the same thing may be made on two computers: a tracked keyword (project + keyword), a tracked place, an AI question, a list keyword. Two copies adding the same keyword end up with one record.
  - **The project's uid** for a project's settings, and one fixed key for the report branding.
  - References travel as the parent's uid and become local ids on arrival. Local ids and addresses do not change. Lists that showed "newest first" by id now sort by time.
- **Changes are found by comparing, not by changing every write.**
  - A scan fingerprints each travelling record (MD5 of its fields, in SQL) and compares with `sync_rows`, the fingerprint it had last time. What differs gets a new change number in this workspace. What disappeared becomes a deletion.
  - Only the deletion of the top record travels: a project's deletion carries its keywords and runs with it. The receiving side deletes the way the program does, so a project goes with everything it holds, its crawls included.
  - On the desktop, a scan runs before each round and when the program closes. On the hub, a scan runs before each answer, so web edits and scheduled results are caught.
- **Large records travel whole and only when finished.**
  - A rank run with its checks, an AI run with its answers, and a project's Search Console and Analytics numbers are fingerprinted by their header (or their numbers) and sent whole.
  - A run still going is never sent. A run going on one computer is therefore never touched by another, and the "interrupted" clean-up stays local.
  - Kept provider answers travel for domains, backlinks, keywords and AI mentions, up to 2 MB each. The newer one wins, and expiry is not a change. A report paid on one side is not paid again on the other. Live result pages stay where they were fetched.
- **Who wins.**
  - Each change carries the hub's change number it was based on. When the hub's record has not changed since, the change is applied as it is.
  - When both sides changed a record, the later change wins (the desktop's clock is corrected by the hub's on every round). A deletion wins over an edit of a record made apart.
  - Times the program writes itself (last rank check, last AI run, last Search Console sync, last report e-mail) keep the later value, whichever side wins. Scheduled work done on one side is therefore not done again on the other.
  - The fingerprint written after taking a change is that of what was stored, so nothing echoes back.
  - A change that arrives before the record it belongs to waits in `sync_pending`, until that record arrives or the sync ends (then it is dropped: the record was deleted).
- **The round.**
  1. A desktop greets the hub (its epoch, the account, how many projects it holds).
  2. It sends its own changes in batches of about 4 MB of JSON, compressed with zstd.
  3. Then it takes everyone else's.
  - It runs every 30 seconds, about 3 seconds after a change made through a command, and after a job ends; it waits up to 5 minutes while the web cannot be reached.
  - Pages are told through a finished `sync` job, and read everything again. On the web, the account's open pages get it through their event stream.
- **What waits for the person.** Three cases stop syncing until the person chooses:
  - the first sync of a computer that has projects, with an account whose web workspace has projects too;
  - another account signed in on a computer that synced before;
  - a hub whose sync records started over (a restore). The hub's epoch changes then.

  The choices are:
  - **merge** (recommended);
  - **put this computer's data on the web** (the hub deletes its projects, and the deletions reach other computers);
  - **take the web's data here**;
  - **pause.**
- **Crawls travel beside the sync.**
  - After a round, the computer that ran an audit sends its crawl folder (zstd, up to 30 MB packed) to `/api/sync/crawls/{audit uid}`. The web keeps up to 5 GB of crawls per account.
  - A desktop takes the crawl of each project's latest audit when it does not have it. Older audits keep their details where they ran.
- **The server's scheduled work pays with a borrowed token.** While the account has not signed in on the web, the server keeps the desktop's token (sealed, and marked as borrowed) for the account's schedules. A later web sign-in replaces it without signing the desktop out, and signing out on the desktop ends it.
- **Report links stay the web's.**
  - A copy cannot make or change a link token, only stop sharing; the server then forgets the link.
  - Views are counted on the web only.
  - Recipients are checked again on the hub, which is the side that sends the e-mails.

## Consequences
- **One place for an account's projects.** The person works on either side and sees the same projects, settings, keywords, questions, reports and measurement history.
- **The desktop is no longer "data stays on this computer" while syncing.** The Settings page and the privacy notes say so, and sync can be switched off.
- **Cost of scanning.** Each round fingerprints the small tables and the Search Console numbers. Checks and answers are not fingerprinted. A round with nothing new costs three small requests.
- **Tombstones are kept.** Deletions stay in `sync_rows` so copies that were away still learn of them. They are small; removing old ones would require a full resync of copies older than them.
- **Not synced.** Own API keys, device settings, spending records, Google sign-ins (each side signs in to Google itself), live result pages, and crawls of older audits.
