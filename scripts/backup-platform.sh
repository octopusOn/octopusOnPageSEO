#!/usr/bin/env bash
# Backs up the web GEO-SEO Platform's data off the server with restic (docs/platform.md,
# "Backups"). Run nightly as root by octopuson-platform-backup.timer (deploy/platform).
#
# Each account's database is copied while its folder's lock is held, the lock the platform takes
# for every operation, so the copy is consistent; the account's other files are copied with it,
# and its crawls after it (a crawl that was running is simply crawled again). The server's own
# server.duckdb is copied as it is, with its write-ahead log. restic then sends the copy
# (RESTIC_REPOSITORY, RESTIC_PASSWORD and the storage's credentials come from
# /etc/octopuson/backup.env) and thins out old snapshots. PLATFORM_MASTER_KEY is not in the data:
# keep it apart, or the backup cannot be opened.
set -euo pipefail

DATA=/var/lib/octopuson/platform
STAGE=/var/lib/octopuson/platform-backup
: "${RESTIC_REPOSITORY:?set RESTIC_REPOSITORY in /etc/octopuson/backup.env}"
command -v restic >/dev/null || { echo "restic is not installed (apt install restic)" >&2; exit 1; }

rm -rf "${STAGE:?}"
install -d -m 700 "$STAGE" "$STAGE/users"
cp -a "$DATA"/server.duckdb* "$STAGE/" 2>/dev/null || true
for account in "$DATA"/users/*/; do
  [ -d "$account" ] || continue
  id=$(basename "$account")
  mkdir -p "$STAGE/users/$id"
  copy=(find "$account" -mindepth 1 -maxdepth 1 ! -name crawls ! -name .lock -exec cp -a {} "$STAGE/users/$id/" ';')
  # Everything but the crawls, under the account's lock (waiting up to 5 minutes for running
  # work). The platform makes the lock file; one made here would belong to root and lock it out.
  if [ -f "$account/.lock" ]; then flock -w 300 "$account/.lock" "${copy[@]}"; else "${copy[@]}"; fi
  if [ -d "$account/crawls" ]; then cp -a "$account/crawls" "$STAGE/users/$id/"; fi
done

restic backup --quiet --tag platform "$STAGE"
restic forget --quiet --tag platform --keep-daily 7 --keep-weekly 4 --keep-monthly 6 --prune
rm -rf "${STAGE:?}"
