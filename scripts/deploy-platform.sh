#!/usr/bin/env bash
# Installs a release of the web GEO-SEO Platform (app.octopuson.com) on the server
# (docs/platform.md, "Deploying"). Run as root on the server:
#
#   bash /home/octopuson/octopusOnPageSEO/scripts/deploy-platform.sh 0.1.0
#
# It pulls main (for the unit and nginx files), downloads the release's Linux server archive from
# GitHub, checks its SHA-256, unpacks it into a new release folder, refreshes the systemd unit and
# the nginx site from deploy/platform and restarts the service. Nothing is built on the server.
# The release must be published: drafts cannot be downloaded. The settings in
# /etc/octopuson/platform.env stay as they are; the first time, create them from
# deploy/platform/platform.env.example.
#
# PLATFORM_RELEASE_REPO says where the release is: this repository (tag platform-v<version>, the
# default) or the program's own project, octopusOn/geo-seo-platform (tag v<version>).
set -euo pipefail

ROOT=/home/octopuson/octopusOnPageSEO
SITE_USER=octopuson
RELEASES=/home/octopuson/platform-releases
ENV_FILE=/etc/octopuson/platform.env
UNIT=octopuson-platform.service
SITE=/etc/nginx/sites-enabled/octopuson-platform.conf
REPO="${PLATFORM_RELEASE_REPO:-octopusOn/octopusOnPageSEO}"
as_site() { sudo -u "$SITE_USER" -H "$@"; }

version="${1:-}"
version="${version#platform-v}"
version="${version#v}"
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "Usage: $0 <version>   (for example 0.1.0)" >&2
  exit 2
fi

cd "$ROOT"
# Pull first, then run the pulled version of this script: bash reads a script as it goes, so it
# must not change under a running copy.
if [ "${2:-}" != "--pulled" ]; then
  as_site git pull --ff-only
  exec bash "$ROOT/scripts/deploy-platform.sh" "$version" --pulled
fi

if [ ! -s "$ENV_FILE" ]; then
  echo "$ENV_FILE is missing: create it from deploy/platform/platform.env.example (owner root, mode 600)." >&2
  exit 1
fi

case "$REPO" in
  */octopusOnPageSEO) tag="platform-v${version}" ;;
  *) tag="v${version}" ;;
esac
# The workflow names the archive after the monorepo tag, wherever the release is.
asset="octopuson-platform-server-platform-v${version}-x86_64-unknown-linux-gnu.tar.gz"
url="https://github.com/${REPO}/releases/download/${tag}/${asset}"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
curl -fsSL -o "$work/$asset" "$url"
curl -fsSL -o "$work/$asset.sha256" "$url.sha256"
(cd "$work" && sha256sum -c "$asset.sha256")

release="$RELEASES/${version}-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$release"
tar xzf "$work/$asset" -C "$release" --strip-components=1 --no-same-owner
test -x "$release/octopuson-platform-server"
test -f "$release/ui/index.html"
chown -R "$SITE_USER:$SITE_USER" "$RELEASES"
as_site ln -sfn "$(basename "$release")" "$RELEASES/current"

if ! cmp -s "$ROOT/deploy/platform/$UNIT" "/etc/systemd/system/$UNIT"; then
  install -m 644 "$ROOT/deploy/platform/$UNIT" "/etc/systemd/system/$UNIT"
  systemctl daemon-reload
  systemctl enable "$UNIT"
fi
if ! cmp -s "$ROOT/deploy/platform/octopuson-platform.conf" "$SITE"; then
  previous=""
  if [ -f "$SITE" ]; then previous="$work/previous.conf"; cp "$SITE" "$previous"; fi
  install -m 644 "$ROOT/deploy/platform/octopuson-platform.conf" "$SITE"
  if ! nginx -t; then
    # Never leave nginx with a site it cannot load: the other sites on this server depend on it.
    if [ -n "$previous" ]; then install -m 644 "$previous" "$SITE"; else rm -f "$SITE"; fi
    echo "nginx refused deploy/platform/octopuson-platform.conf; the previous site is back." >&2
    exit 1
  fi
  systemctl reload nginx
fi
systemctl restart octopuson-platform

# Keep the three newest releases; to roll back, point `current` at an older one and restart.
current=$(readlink "$RELEASES/current")
find "$RELEASES" -mindepth 1 -maxdepth 1 -type d -name '[0-9]*' -printf '%T@ %f\n' | sort -rn | tail -n +4 | cut -d' ' -f2- |
  while read -r old; do [ "$old" = "$current" ] || rm -rf "${RELEASES:?}/$old"; done

sleep 2
curl -fsS -o /dev/null -w "platform %{http_code}\n" http://127.0.0.1:8796/health
