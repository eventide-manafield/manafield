#!/usr/bin/env bash
set -u

if [ "$#" -ne 0 ]; then
  echo "usage: manafield-status" >&2
  exit 64
fi

CORE_URL="${MANAFIELD_CORE_URL:-http://127.0.0.1:18080}"
REFERENCE_URL="${MANAFIELD_REFERENCE_URL:-http://127.0.0.1:18081}"
REFERENCE_MODULE_ID="${MANAFIELD_REFERENCE_MODULE_ID:-reference-web}"

failed=0

section() {
  printf '\n== %s ==\n' "$1"
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

check_registry_module() {
  label="$1"
  url="$2"
  module_id="$3"

  response="$(curl --fail --silent --show-error --max-time 3 "$url" 2>&1)"
  status=$?

  if [ "$status" -ne 0 ]; then
    printf '[fail] %s\n%s\n' "$label" "$response"
    failed=1
    return
  fi

  if printf '%s' "$response" | grep -Fq "\"id\":\"$module_id\""; then
    printf '[ok] %s: %s\n' "$label" "$module_id"
  else
    printf '[fail] %s: module %s not found\n' "$label" "$module_id"
    printf '%s\n' "$response"
    failed=1
  fi
}

section "Manafield Core"
check_http "Core health" "$CORE_URL/health"
check_registry_module "Core registry contains reference module" "$CORE_URL/modules" "$REFERENCE_MODULE_ID"

section "Reference Module"
check_http "Reference health" "$REFERENCE_URL/manafield/health"
check_registry_module "Reference can read Core registry" "$REFERENCE_URL/api/core/modules" "$REFERENCE_MODULE_ID"

section "Result"
if [ "$failed" -eq 0 ]; then
  echo "Manafield deployment is healthy."
else
  echo "Manafield deployment has one or more failures."
fi

exit "$failed"
