#!/usr/bin/env bash
set -euo pipefail

PROJECT="manafield"

for service in core reference-web; do
  container_id="$(
    docker ps       --filter "label=com.docker.compose.project=$PROJECT"       --filter "label=com.docker.compose.service=$service"       --format '{{.ID}}'       | head -n 1
  )"

  printf '\n===== %s =====\n' "$service"

  if [ -z "$container_id" ]; then
    echo "No running container found."
    continue
  fi

  docker logs     --tail 100     --timestamps     "$container_id"     2>&1
done
