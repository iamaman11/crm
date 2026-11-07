#!/usr/bin/env bash
# Never restores into the live project. Requires a distinct Compose project.
set -euo pipefail
umask 077
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
archive="${1:?usage: restore.sh /absolute/path/crm.dump crm-restore-name}"
[[ -f "$archive" ]] || { echo "Backup archive not found" >&2; exit 2; }
archive="$(cd "$(dirname "$archive")" && pwd -P)/$(basename "$archive")"
project="${2:?usage: restore.sh /absolute/path/crm.dump crm-restore-name}"
[[ "$project" =~ ^crm-restore-[a-z0-9-]+$ ]] || {
  echo "Restore project must start with crm-restore- and contain only lowercase/dashes" >&2
  exit 2
}
[[ -f "$archive" && -f "$archive.sha256" ]] || {
  echo "Missing archive or .sha256 checksum" >&2; exit 2
}
(cd "$(dirname "$archive")" && sha256sum -c "$(basename "$archive").sha256")
c=(docker compose --project-name "$project" --env-file "$here/.env" -f "$here/compose.yaml")
"${c[@]}" up --detach --wait database
existing="$("${c[@]}" exec -T database psql -U postgres -d crm -qAt -c \
  "SELECT count(*) FROM pg_catalog.pg_tables WHERE schemaname = 'crm'")"
[[ "$existing" == "0" ]] || {
  echo "Refusing to restore to a database with existing CRM schema" >&2
  exit 3
}
"${c[@]}" exec -T database pg_restore -U postgres -d crm \
  --exit-on-error --single-transaction --no-owner --no-acl <"$archive"
# Logical archives omit cluster roles. Reapply migration/role setup in the isolated copy.
"${c[@]}" run --rm migrate
runtime_role="$("${c[@]}" exec -T database psql -X -U postgres -d crm -qAt -v ON_ERROR_STOP=1 -c \
  "SELECT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'crm_runtime' AND rolcanlogin AND NOT rolbypassrls)")"
[[ "$runtime_role" == "t" ]] || {
  echo "Restored runtime role is absent or violates LOGIN/NOBYPASSRLS" >&2
  exit 4
}
"${c[@]}" exec -T database psql -U postgres -d crm -qAt -c \
  "SELECT 'records=' || count(*) FROM crm.records"
echo "Independent database restore finished into project $project."
echo "Keep source project running; verify business, tenant and audit consistency separately."
