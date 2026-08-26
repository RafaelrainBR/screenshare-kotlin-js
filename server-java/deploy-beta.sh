#!/usr/bin/env bash
#
# Deploy the BETA/CANARY environment to Fly.io.
#
# This deploys to the SEPARATE app `screen-share-beta` (see fly.beta.toml),
# so testing new features never affects the production app (`screen-share`).
#
# Usage (from the repo root or anywhere):
#   ./server-java/deploy-beta.sh
#
# Requirements:
#   - flyctl installed and authenticated (fly auth login)
#   - You are on the branch you want to deploy (e.g. beta)
#
# First run creates the app and allocates an IPv4 address automatically.

set -euo pipefail

APP_NAME="screen-share-beta"
CONFIG_FILE="server-java/fly.beta.toml"

cd "$(dirname "$0")/.." # go to repo root so the config path resolves

echo "==> Deploying beta app '$APP_NAME' using '$CONFIG_FILE'"

if ! fly apps list 2>/dev/null | grep -q "^$APP_NAME[[:space:]]"; then
  echo "==> App '$APP_NAME' not found. Creating it..."
  fly apps create "$APP_NAME"
fi

if ! fly ips list -a "$APP_NAME" 2>/dev/null | grep -q "IPv4"; then
  echo "==> No IPv4 address allocated. Allocating one..."
  fly ips allocate-v4 -a "$APP_NAME"
fi

echo "==> Building and deploying..."
fly deploy -c "$CONFIG_FILE"

echo "==> Done! Beta app available at https://$APP_NAME.fly.dev"
