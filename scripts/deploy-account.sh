#!/usr/bin/env bash
# Updates the live account portal (my.octopuson.com) and credits gateway (api.octopuson.com) on
# the server (docs/cloud.md, "Deploying"). Run as root on the server:
#
#   bash /home/octopuson/octopusOnPageSEO/scripts/deploy-account.sh
#
# It pulls main, installs dependencies, applies the database migrations, builds the portal into a
# new release directory, refreshes the systemd units from deploy/account and restarts both
# services. The settings in /etc/octopuson and the nginx vhost stay as they are. The portal reads
# its languages and legal texts from the CMS, so deploy the CMS first when both changed
# (scripts/deploy-website.sh).
set -euo pipefail

ROOT=/home/octopuson/octopusOnPageSEO
SITE_USER=octopuson
NODE_BIN=/opt/node-v22.23.3-linux-x64/bin
RELEASES=/home/octopuson/portal-releases
as_site() { sudo -u "$SITE_USER" -H env PATH="$NODE_BIN:$PATH" COREPACK_ENABLE_DOWNLOAD_PROMPT=0 NEXT_TELEMETRY_DISABLED=1 CI=1 "$@"; }

cd "$ROOT"
as_site git pull --ff-only
as_site corepack pnpm install --frozen-lockfile --ignore-scripts --filter "@octopusseo/portal..." --filter "@octopusseo/gateway..."

# Migrations first, so the new code finds its tables.
bash "$ROOT/scripts/account-cli.sh" migrate

# The portal runs from its own copy of the build, so building never disturbs the live server.
as_site corepack pnpm --filter @octopusseo/portal build
release="$RELEASES/$(date +%Y%m%d-%H%M%S)"
as_site mkdir -p "$RELEASES"
as_site cp -a "$ROOT/apps/portal/.next/standalone" "$release"
as_site cp -a "$ROOT/apps/portal/.next/static" "$release/apps/portal/.next/static"
as_site cp -a "$ROOT/apps/portal/messages" "$release/apps/portal/"
if [ -d "$ROOT/apps/portal/public" ]; then as_site cp -a "$ROOT/apps/portal/public" "$release/apps/portal/public"; fi
as_site ln -sfn "$(basename "$release")" "$RELEASES/current"

for unit in octopuson-portal.service octopuson-gateway.service; do
  if ! cmp -s "$ROOT/deploy/account/$unit" "/etc/systemd/system/$unit"; then
    install -m 644 "$ROOT/deploy/account/$unit" "/etc/systemd/system/$unit"
    systemctl daemon-reload
    systemctl enable "$unit"
  fi
done
systemctl restart octopuson-portal octopuson-gateway

# Keep the three newest releases; to roll back, point `current` at an older one and restart.
find "$RELEASES" -mindepth 1 -maxdepth 1 -type d -name '20*' | sort | head -n -3 | xargs -r rm -rf

sleep 3
curl -fsS -o /dev/null -w "portal %{http_code}\n" http://127.0.0.1:3030/giris
curl -fsS -o /dev/null -w "gateway %{http_code}\n" http://127.0.0.1:8787/health
