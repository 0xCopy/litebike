#!/usr/bin/env bash
set -euo pipefail

# Idempotent sync script to fetch a termux remote over SSH port 8022 and create local tracking
# branches under refs/heads/termux/*. This script does NOT push or overwrite origin.

REMOTE_NAME=${REMOTE_NAME:-termux-temp}
REMOTE_URL=${REMOTE_URL:-ssh://u0_a471@192.168.21.199:8022/~/litebike.git}

echo "[sync_termux] ensuring remote $REMOTE_NAME -> $REMOTE_URL"
git remote remove "$REMOTE_NAME" 2>/dev/null || true
git remote add "$REMOTE_NAME" "$REMOTE_URL"

echo "[sync_termux] fetching from $REMOTE_NAME using SSH port 8022"
GIT_SSH_COMMAND="ssh -p 8022" git fetch --prune "$REMOTE_NAME"

echo "[sync_termux] creating/updating local tracking branches under refs/heads/termux/"
mkdir -p .git/refs/heads/termux 2>/dev/null || true
for ref in $(git for-each-ref --format='%(refname:short)' refs/remotes/$REMOTE_NAME 2>/dev/null || true); do
  name=${ref#${REMOTE_NAME}/}
  local_ref=termux/${name}
  echo " - $local_ref -> $ref"
  # create or force-update local branch to point at the remote ref
  git branch --force "$local_ref" "$ref"
done

echo "[sync_termux] done. Local termux/* branches updated. No push performed."
