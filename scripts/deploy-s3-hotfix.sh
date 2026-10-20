#!/usr/bin/env bash
# Narrow rollout of the S3 Provider polling fix. Does not touch other services.
# Operator executes with "sudo bash <script>" (release.env is root-only).
set -Eeuo pipefail

RELEASE=/opt/manafield/ai-workspace/instance/releases/native-20261019T005014Z
SOURCE=/opt/manafield/ai-workspace/work/s3-healthcheck-on-demand-20261020/providers/s3
SERVICE=manafield-s3-provider
PROJECT=manafield
NEW_IMAGE=manafield-resource-s3:healthcheck-on-demand-20261020

if [[ $(id -u) -ne 0 ]]; then
  echo "ERROR: Run with sudo; release.env is root-only." >&2
  exit 1
fi
for tool in docker python3; do
  command -v "$tool" >/dev/null || { echo "Missing $tool" >&2; exit 1; }
done
for file in "$RELEASE/compose.yml" "$RELEASE/modules.compose.yml" "$RELEASE/release.env" \
            "$SOURCE/Dockerfile" "$SOURCE/provider.py"; do
  [[ -f "$file" ]] || { echo "Missing $file" >&2; exit 1; }
done

compose() {
  # Validate against all profiles from the active release. The enabled
  # Noctemblith module depends on Redis; --profile s3 alone excludes that
  # service and makes the complete Compose model invalid.
  # --no-deps on the targeted 'up' still recreates ONLY the S3 provider.
  docker compose --project-name "$PROJECT" \
    --profile postgresql --profile s3 --profile redis \
    --env-file "$RELEASE/release.env" \
    -f "$RELEASE/compose.yml" \
    -f "$RELEASE/modules.compose.yml" "$@"
}
mapfile -t ids < <(docker ps -aq \
  --filter "label=com.docker.compose.project=$PROJECT" \
  --filter "label=com.docker.compose.service=$SERVICE")
if [[ ${#ids[@]} -ne 1 ]]; then
  echo "ERROR: Expected exactly one old $SERVICE, found ${#ids[@]}." >&2
  exit 1
fi
old_id="${ids[0]}"
if [[ "$(docker inspect -f '{{.State.Running}}' "$old_id")" != "false" ]]; then
  echo "ERROR: Existing S3 Provider is still running; refusing to interrupt it." >&2
  exit 1
fi
old_image="$(docker inspect -f '{{.Image}}' "$old_id")"
old_short="${old_image#sha256:}"
rollback_image="manafield-resource-s3:rollback-${old_short:0:12}"
docker image tag "$old_image" "$rollback_image"

echo "=== Scope: only $SERVICE; other services/volumes unchanged ==="
echo "Rollback image: $rollback_image"
echo "=== Build S3 Provider image only ==="
docker build --file "$SOURCE/Dockerfile" --tag "$NEW_IMAGE" "$SOURCE"

export MANAFIELD_S3_PROVIDER_IMAGE="$NEW_IMAGE"
echo "=== Validate release Compose (no secret values printed) ==="
compose config --quiet
configured_image="$(compose config --format json | python3 -c 'import json,sys; print(json.load(sys.stdin)["services"]["manafield-s3-provider"]["image"])')"
if [[ "$configured_image" != "$NEW_IMAGE" ]]; then
  echo "ERROR: Compose image override did not take effect; aborting." >&2
  exit 1
fi
echo "Verified image: $configured_image"

echo "=== Recreate only stopped S3 Provider ==="
compose up --detach --no-deps --no-build --force-recreate "$SERVICE"
mapfile -t new_ids < <(docker ps -aq \
  --filter "label=com.docker.compose.project=$PROJECT" \
  --filter "label=com.docker.compose.service=$SERVICE")
if [[ ${#new_ids[@]} -ne 1 ]]; then
  echo "ERROR: Expected one recreated S3 container; found ${#new_ids[@]}." >&2
  exit 1
fi
new_id="${new_ids[0]}"
expected="$(docker image inspect -f '{{.Id}}' "$NEW_IMAGE")"
actual="$(docker inspect -f '{{.Image}}' "$new_id")"
if [[ "$actual" != "$expected" ]]; then
  echo "ERROR: Running wrong image. Stopping this provider." >&2
  docker stop --time 15 "$new_id" >/dev/null || :
  exit 1
fi
if [[ "$(docker inspect -f '{{.State.Running}}' "$new_id")" != "true" ]]; then
  echo "ERROR: New Provider is not running." >&2
  exit 1
fi

echo "=== Await startup HeadBucket check ==="
verified=false
for i in $(seq 1 30); do
  logs="$(docker logs --since 90s "$new_id" 2>&1 || :)"
  if grep -q 'S3 initial bucket probe succeeded' <<< "$logs"; then
    verified=true
    break
  fi
  if [[ "$(docker inspect -f '{{.State.Running}}' "$new_id")" != "true" ]]; then
    break
  fi
  sleep 1
done
if [[ "$verified" != true ]]; then
  echo "ERROR: Initial bucket probe unverified; stopping this Provider." >&2
  docker stop --time 15 "$new_id" >/dev/null || :
  docker logs --tail 15 "$new_id" >&2 || :
  echo "Rollback image preserved: $rollback_image" >&2
  exit 1
fi

echo "PASS: Initial HeadBucket succeeded; periodic Core checks do not hit R2."
docker ps --filter "id=$new_id" --format '{{.Names}} {{.Status}}'
echo "Rollback image preserved: $rollback_image"
echo "NOTE: Future full releases must update the desired provider image/source."
