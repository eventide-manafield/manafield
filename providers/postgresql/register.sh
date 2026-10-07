#!/bin/sh
set -eu

CORE_URL="${MANAFIELD_CORE_URL:-http://core:8080}"
RESOURCE_ID="${MANAFIELD_RESOURCE_ID:-example-postgres}"
RESOURCE_NAME="${MANAFIELD_RESOURCE_NAME:-Example PostgreSQL}"
POSTGRES_HOST="${POSTGRES_HOST:-postgres}"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"
CAPABILITY_VERSION="${MANAFIELD_POSTGRES_CAPABILITY_VERSION:-1.0.0}"
CHECK_INTERVAL="${MANAFIELD_RESOURCE_CHECK_INTERVAL:-5}"

payload() {
  cat <<EOF
{
  "id": "$RESOURCE_ID",
  "name": "$RESOURCE_NAME",
  "type": "postgresql",
  "description": "PostgreSQL Resource registered by the Manafield PostgreSQL Resource Provider.",
  "provides": {
    "capabilities": [
      {
        "id": "database.postgresql",
        "version": "$CAPABILITY_VERSION"
      }
    ]
  }
}
EOF
}

resource_status() {
  curl     --silent     --output /dev/null     --write-out '%{http_code}'     "$CORE_URL/resources/$RESOURCE_ID"     2>/dev/null || true
}

register_resource() {
  body="$(payload)"
  response_file="$(mktemp)"
  trap 'rm -f "$response_file"' EXIT

  status="$(
    curl       --silent       --show-error       --output "$response_file"       --write-out '%{http_code}'       --header 'Content-Type: application/json'       --data "$body"       "$CORE_URL/resources"       2>/dev/null || true
  )"

  case "$status" in
    201)
      echo "Registered Resource '$RESOURCE_ID' in Manafield Core"
      ;;
    409)
      echo "Resource '$RESOURCE_ID' is already registered"
      ;;
    *)
      echo "Resource registration is not available yet (HTTP ${status:-000})" >&2
      return 1
      ;;
  esac

  rm -f "$response_file"
  trap - EXIT
}

echo "PostgreSQL Resource Provider starting"
echo "Resource: $RESOURCE_ID"
echo "PostgreSQL: $POSTGRES_HOST:$POSTGRES_PORT"
echo "Manafield Core: $CORE_URL"

while true; do
  if pg_isready -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" >/dev/null 2>&1; then
    case "$(resource_status)" in
      200)
        ;;
      404)
        register_resource || true
        ;;
      *)
        register_resource || true
        ;;
    esac
  else
    echo "PostgreSQL is not ready at $POSTGRES_HOST:$POSTGRES_PORT" >&2
  fi

  sleep "$CHECK_INTERVAL"
done