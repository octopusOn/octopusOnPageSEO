#!/usr/bin/env bash
# Runs the credits database chores (packages/cloud-core/src/cli.ts) on the server with the
# services' settings from /etc/octopuson/cloud.env (docs/cloud.md, "Deploying"). Run as root:
#
#   bash /home/octopuson/octopusOnPageSEO/scripts/account-cli.sh migrate
#   bash /home/octopuson/octopusOnPageSEO/scripts/account-cli.sh seed
#   bash /home/octopuson/octopusOnPageSEO/scripts/account-cli.sh promote you@example.com
#   bash /home/octopuson/octopusOnPageSEO/scripts/account-cli.sh adjust you@example.com 5000 "reason"
#   bash /home/octopuson/octopusOnPageSEO/scripts/account-cli.sh cache-rules
#
# systemd reads the settings file and starts the command as the site user, so the database
# address and the master key never appear on a command line.
set -euo pipefail

ROOT=/home/octopuson/octopusOnPageSEO
SITE_USER=octopuson
NODE=/opt/node-v22.23.3-linux-x64/bin/node

exec systemd-run --quiet --wait --pipe --collect \
  --uid="$SITE_USER" --gid="$SITE_USER" \
  --working-directory="$ROOT/packages/cloud-core" \
  --property=EnvironmentFile=/etc/octopuson/cloud.env \
  "$NODE" src/cli.ts "$@"
