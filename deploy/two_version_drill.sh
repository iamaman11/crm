#!/usr/bin/env bash
# CI-only compatibility proof: distinct G2 and G3 crm-api binaries against same persistent data.
# Never run this test against a real customer deployment.
set -euo pipefail
umask 077
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
baseline="4ec3b416d77857347f797f185ad3981a95219b04"
[[ -n "${GITHUB_RUN_ID:-}" && -n "${RUNNER_TEMP:-}" ]] || {
  echo "Two-version drill is restricted to disposable GitHub Actions runners" >&2
  exit 2
}
[[ -f "$here/.env" ]] || exit 2
git -C "$root" cat-file -e "$baseline^{commit}"
git -C "$root" merge-base --is-ancestor "$baseline" HEAD
work="$(mktemp -d "$RUNNER_TEMP/crm-g3-two-version.XXXXXX")"
old_tag="crm-g3-g2-baseline-api:$GITHUB_RUN_ID"
new_tag="crm-g3-g3-current-api:$GITHUB_RUN_ID"
cleanup() {
  git -C "$root" worktree remove --force "$work/previous-source" >/dev/null 2>&1 || true
  rm -rf "$work"
  docker image rm "$old_tag" "$new_tag" >/dev/null 2>&1 || true
}
trap cleanup EXIT
c=(docker compose --env-file "$here/.env" -f "$here/compose.yaml")
new_container="$("${c[@]}" ps -q api)"
[[ -n "$new_container" ]] || exit 2
new_id="$(docker inspect --format '{{.Image}}' "$new_container")"
[[ "$new_id" == sha256:* ]] || exit 2
docker image tag "$new_id" "$new_tag"

# G2 predates G3 packaging and contract-export argument. Build the genuine
# immutable accepted G2 source with the same base runtime; do not patch business code.
git -C "$root" worktree add --detach "$work/previous-source" "$baseline"
cat >"$work/previous-api.Dockerfile" <<'DOCKER'
FROM rust:1.97.1-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build --release --locked -p crm-api
FROM debian:bookworm-slim AS api
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/crm-api /usr/local/bin/crm-api
USER 65532:65532
EXPOSE 8080 9090
ENTRYPOINT ["crm-api"]
DOCKER
docker build --file "$work/previous-api.Dockerfile" --target api \
  --tag "$old_tag" "$work/previous-source"
old_id="$(docker image inspect --format '{{.Id}}' "$old_tag")"
[[ "$old_id" == sha256:* && "$old_id" != "$new_id" ]] || {
  echo "Historical and current API images must differ" >&2; exit 3;
}
old_binary="$(docker run --rm --entrypoint sha256sum "$old_tag" /usr/local/bin/crm-api | cut -d ' ' -f1)"
new_binary="$(docker run --rm --entrypoint sha256sum "$new_tag" /usr/local/bin/crm-api | cut -d ' ' -f1)"
[[ "$old_binary" =~ ^[0-9a-f]{64}$ && "$new_binary" =~ ^[0-9a-f]{64}$ ]] || exit 3
[[ "$old_binary" != "$new_binary" ]] || {
  echo "Historical and current application binaries are byte-for-byte identical" >&2; exit 3;
}
echo "Verified distinct compiled crm-api binaries from immutable G2 and G3 source."

# Never touch the database service, migration journal or Caddy website.
printf 'services:\n  api:\n    image: %s\n' "$old_tag" >"$work/g2.yml"
previous=(docker compose --env-file "$here/.env" -f "$here/compose.yaml" -f "$work/g2.yml")
"${previous[@]}" up -d --no-build --no-deps --force-recreate --wait api
[[ "$(docker inspect --format '{{.Image}}' "$("${c[@]}" ps -q api)")" == "$old_id" ]] || exit 4
echo "PASS: G3 -> accepted G2 application binary switched without database rollback."
echo "G3_TWO_VERSION_PHASE=previous"

# Browser acceptance is called by the parent workflow here. Keep G2 running
# until it successfully reads data; restoration of G3 is a separate explicit step.
printf '%s\n' "$new_id" >"$RUNNER_TEMP/g3-current-api-image-id"
printf '%s\n' "$old_id" >"$RUNNER_TEMP/g3-previous-api-image-id"
printf '%s\n' "$old_binary" >"$RUNNER_TEMP/g3-previous-api-binary-sha256"
printf '%s\n' "$new_binary" >"$RUNNER_TEMP/g3-current-api-binary-sha256"
printf 'services:\n  api:\n    image: %s\n' "$new_tag" >"$RUNNER_TEMP/g3-new-api.yml"
# The old and current image references must survive this script, because the
# workflow needs to run its real browser checks between switches.
trap - EXIT
git -C "$root" worktree remove --force "$work/previous-source"
rm -rf "$work"
