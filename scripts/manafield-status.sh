#!/usr/bin/env bash
set -u

if [ "$#" -ne 0 ]; then
  echo "usage: manafield-status" >&2
  exit 64
fi

CORE_URL="http://127.0.0.1:18080"
REFERENCE_URL="http://127.0.0.1:18081"
PROJECT="manafield"

failed=0

section() {
  printf '\n== %s ==\n' "$1"
}

check_container() {
  service="$1"
  result="$(
    docker ps       --filter "label=com.docker.compose.project=$PROJECT"       --filter "label=com.docker.compose.service=$service"       --format '{{.Names}} | {{.Status}}'       2>/dev/null
  )"

  if [ -n "$result" ]; then
    printf '[ok] %s: %s\n' "$service" "$result"
  else
    printf '[fail] %s: no running container found\n' "$service"
    failed=1
  fi
}

check_http() {
  label="$1"
  url="$2"

  response="$(curl --fail --silent --show-error --max-time 3 "$url" 2>&1)"
  status=$?

  if [ "$status" -eq 0 ]; then
    printf '[ok] %s\n%s\n' "$label" "$response"
  else
    printf '[fail] %s\n%s\n' "$label" "$response"
    failed=1
  fi
}

section "Manafield containers"
check_container core
check_container reference-web

section "Manafield endpoints"
check_http "Core health" "$CORE_URL/health"
check_http "Core registry" "$CORE_URL/modules"
check_http "Reference health" "$REFERENCE_URL/manafield/health"
check_http "Reference -> Core registry" "$REFERENCE_URL/api/core/modules"

section "Result"
if [ "$failed" -eq 0 ]; then
  echo "Manafield deployment looks healthy."
else
  echo "Manafield deployment has one or more failures."
fi

exit "$failed"
