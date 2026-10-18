#!/usr/bin/env bash
# Updates the live website and content admin on the server (docs/cms.md, "Deploying").
# Run as root on the server:
#
#   bash /home/octopuson/octopusOnPageSEO/scripts/deploy-website.sh
#
# It pulls main, installs dependencies, migrates the CMS, rebuilds both apps and restarts the
# website. The CMS database, uploads and both .env files are not in git and stay as they are.
set -euo pipefail

ROOT=/home/octopuson/octopusOnPageSEO
SITE_USER=octopuson
PHP=php8.5
NODE_BIN=/opt/node-v22.23.3-linux-x64/bin
as_site() { sudo -u "$SITE_USER" -H env PATH="$NODE_BIN:$PATH" COREPACK_ENABLE_DOWNLOAD_PROMPT=0 NEXT_TELEMETRY_DISABLED=1 CI=1 "$@"; }

cd "$ROOT"
as_site git pull --ff-only
as_site corepack pnpm install --frozen-lockfile --ignore-scripts --filter "@octopusseo/web..." --filter "@octopusseo/cms..."

cd "$ROOT/apps/cms"
as_site "$PHP" /usr/local/bin/composer install --no-dev --optimize-autoloader --no-interaction --no-progress
as_site "$PHP" artisan migrate --force
cd "$ROOT"
as_site corepack pnpm --filter @octopusseo/cms build
cd "$ROOT/apps/cms"
as_site "$PHP" artisan optimize

# The website's build reads every page from the CMS (127.0.0.1:8095).
cd "$ROOT"
as_site corepack pnpm --filter @octopusseo/web build
systemctl restart octopuson-web
sleep 3
curl -fsS -o /dev/null -w "website %{http_code}\n" http://127.0.0.1:3000/en
