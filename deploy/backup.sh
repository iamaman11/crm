#!/usr/bin/env bash
# Run from any directory. Files stay on the operator host, never in CI artifacts.
set -euo pipefail
umask 077
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
destination="${1:-$here/backups}"
mkdir -p "$destination"
[[ -d "$destination" ]] || exit 2
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
file="$destination/crm-$stamp.dump"
[[ ! -e "$file" ]] || { echo "Backup already exists" >&2; exit 2; }
docker compose --env-file "$here/.env" -f "$here/compose.yaml" exec -T database \
    pg_dump -U postgres -d crm -Fc >"$file"
test -s "$file" || { rm -f "$file"; echo "Empty database backup" >&2; exit 1; }
sha256sum "$file" >"$file.sha256"
echo "Backup created: $file"
echo "Verify an independent restore before treating this as a recovery point."
