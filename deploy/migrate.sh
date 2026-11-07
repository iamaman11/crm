#!/usr/bin/env bash
# One-shot, non-destructive schema migration and runtime DB-role setup.
set -euo pipefail
umask 077
: "${PGHOST:?}"
: "${PGUSER:?}"
: "${PGDATABASE:?}"
: "${PGPASSWORD:?}"
: "${CRM_APP_DB_PASSWORD:?}"
[[ "${CRM_APP_DB_PASSWORD}" =~ ^[[:xdigit:]]{64}$ ]] || {
  echo "CRM_APP_DB_PASSWORD must be 64 hexadecimal characters" >&2
  exit 2
}
psql -X -q -v ON_ERROR_STOP=1 <<'SQL'
CREATE TABLE IF NOT EXISTS public.crm_schema_migrations (
    file_name text PRIMARY KEY,
    sha256 text NOT NULL CHECK (length(sha256) = 64),
    applied_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
SQL

for file in /migrations/*.up.sql; do
  [[ -f "$file" ]] || { echo "No schema migrations found" >&2; exit 2; }
  name="${file##*/}"
  [[ "$name" =~ ^[a-zA-Z0-9_\.\-]+$ ]] || { echo "Unexpected migration name" >&2; exit 2; }
  sha="$(sha256sum "$file" | cut -d ' ' -f1)"
  old="$(psql -X -qAt -v ON_ERROR_STOP=1 -c "SELECT sha256 FROM public.crm_schema_migrations WHERE file_name = '$name'")"
  if [[ "$old" == "$sha" ]]; then
    continue
  fi
  if [[ -n "$old" ]]; then
    echo "Migration was modified after applying: $name" >&2
    exit 3
  fi
  if [[ "$(head -n 1 "$file")" != "BEGIN;" || "$(tail -n 1 "$file")" != "COMMIT;" ]]; then
    echo "Migration does not have expected BEGIN/COMMIT wrapper: $name" >&2
    exit 3
  fi
  {
    echo "BEGIN;"
    sed '1d;$d' "$file"
    printf "INSERT INTO public.crm_schema_migrations (file_name, sha256) VALUES ('%s', '%s');\n" "$name" "$sha"
    echo "COMMIT;"
  } | psql -X -q -v ON_ERROR_STOP=1
  echo "Applied $name"
done

# Do not reuse crm_app_test (test fixtures) and do not grant BYPASSRLS.
psql -X -q -v ON_ERROR_STOP=1 -v db_password="$CRM_APP_DB_PASSWORD" <<'SQL'
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'crm_runtime') THEN
    CREATE ROLE crm_runtime LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOBYPASSRLS;
  END IF;
END
$$;
SELECT format('ALTER ROLE crm_runtime LOGIN PASSWORD %L', :'db_password') \gexec
GRANT CONNECT ON DATABASE crm TO crm_runtime;
GRANT USAGE ON SCHEMA crm TO crm_runtime;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA crm TO crm_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA crm TO crm_runtime;
REVOKE INSERT, UPDATE, DELETE ON crm.audit_heads FROM crm_runtime;
SQL
echo "Schema migration and dedicated FORCE-RLS runtime role provisioned."
