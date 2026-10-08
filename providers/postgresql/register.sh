#!/bin/sh
set -eu

CORE_URL="${MANAFIELD_CORE_URL:-http://core:8080}"
RESOURCE_ID="${MANAFIELD_RESOURCE_ID:-manafield-postgres}"
RESOURCE_NAME="${MANAFIELD_RESOURCE_NAME:-Manafield PostgreSQL}"
POSTGRES_HOST="${POSTGRES_HOST:-manafield-postgres}"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"
POSTGRES_DB="${POSTGRES_DB:-manafield}"
POSTGRES_USER="${POSTGRES_USER:-manafield}"
ALLOCATIONS_FILE="${MANAFIELD_POSTGRES_ALLOCATIONS_FILE:-/run/manafield/postgresql/allocations.tsv}"
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
      if [ "$(resource_status)" = "200" ]; then
        echo "Resource '$RESOURCE_ID' is already registered"
      else
        echo "Instance ID '$RESOURCE_ID' is already used by another registered instance" >&2
        return 1
      fi
      ;;
    *)
      echo "Resource registration is not available yet (HTTP ${status:-000})" >&2
      return 1
      ;;
  esac

  rm -f "$response_file"
  trap - EXIT
}

valid_postgres_identifier() {
  printf '%s' "$1" | grep -Eq '^[a-z_][a-z0-9_]*$'
}

provision_allocation() {
  database="$1"
  schema="$2"
  username="$3"
  password_file="$4"

  if ! valid_postgres_identifier "$database"; then
    echo "Invalid PostgreSQL database identifier '$database'" >&2
    return 1
  fi

  if ! valid_postgres_identifier "$schema"; then
    echo "Invalid PostgreSQL schema identifier '$schema'" >&2
    return 1
  fi

  if ! valid_postgres_identifier "$username"; then
    echo "Invalid PostgreSQL role identifier '$username'" >&2
    return 1
  fi

  if [ ! -r "$password_file" ]; then
    echo "PostgreSQL binding password file is not readable: $password_file" >&2
    return 1
  fi

  password="$(tr -d '\r\n' < "$password_file")"

  case "$password" in
    ""|*[!0-9a-f]*)
      echo "PostgreSQL binding password must be a non-empty lowercase hex secret" >&2
      return 1
      ;;
  esac

  database_exists="$(
    psql       -h "$POSTGRES_HOST"       -p "$POSTGRES_PORT"       -U "$POSTGRES_USER"       -d "$POSTGRES_DB"       -tAc "SELECT 1 FROM pg_database WHERE datname = '$database'"       | tr -d '[:space:]'
  )"

  if [ "$database_exists" != "1" ]; then
    echo "PostgreSQL Resource database '$database' does not exist" >&2
    return 1
  fi

  role_exists="$(
    psql       -h "$POSTGRES_HOST"       -p "$POSTGRES_PORT"       -U "$POSTGRES_USER"       -d "$POSTGRES_DB"       -tAc "SELECT 1 FROM pg_roles WHERE rolname = '$username'"       | tr -d '[:space:]'
  )"

  if [ "$role_exists" = "1" ]; then
    psql       -h "$POSTGRES_HOST"       -p "$POSTGRES_PORT"       -U "$POSTGRES_USER"       -d "$POSTGRES_DB"       -v ON_ERROR_STOP=1       -c "ALTER ROLE \"$username\" WITH LOGIN PASSWORD '$password'"       >/dev/null
  else
    psql       -h "$POSTGRES_HOST"       -p "$POSTGRES_PORT"       -U "$POSTGRES_USER"       -d "$POSTGRES_DB"       -v ON_ERROR_STOP=1       -c "CREATE ROLE \"$username\" WITH LOGIN PASSWORD '$password'"       >/dev/null
  fi

  psql     -h "$POSTGRES_HOST"     -p "$POSTGRES_PORT"     -U "$POSTGRES_USER"     -d "$database"     -v ON_ERROR_STOP=1     -c "CREATE SCHEMA IF NOT EXISTS \"$schema\" AUTHORIZATION \"$username\""     -c "ALTER SCHEMA \"$schema\" OWNER TO \"$username\""     -c "REVOKE ALL ON SCHEMA \"$schema\" FROM PUBLIC"     -c "GRANT CONNECT ON DATABASE \"$database\" TO \"$username\""     -c "GRANT USAGE, CREATE ON SCHEMA \"$schema\" TO \"$username\""     -c "ALTER ROLE \"$username\" IN DATABASE \"$database\" SET search_path TO \"$schema\""     >/dev/null

  echo "Provisioned PostgreSQL schema '$schema' in database '$database' for role '$username'"
}

provision_allocations() {
  if [ ! -s "$ALLOCATIONS_FILE" ]; then
    echo "No PostgreSQL allocations requested"
    return 0
  fi

  tab="$(printf '\t')"

  while IFS="$tab" read -r allocation_resource database schema username password_file; do
    [ -n "$allocation_resource" ] || continue
    [ "$allocation_resource" = "$RESOURCE_ID" ] || continue

    provision_allocation "$database" "$schema" "$username" "$password_file"
  done < "$ALLOCATIONS_FILE"
}

echo "PostgreSQL Resource Provider starting"
echo "Resource: $RESOURCE_ID"
echo "PostgreSQL: $POSTGRES_HOST:$POSTGRES_PORT"
echo "Manafield Core: $CORE_URL"

allocations_provisioned=false

while true; do
  if pg_isready -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" >/dev/null 2>&1; then
    if [ "$allocations_provisioned" = "false" ]; then
      if provision_allocations; then
        allocations_provisioned=true
      else
        echo "PostgreSQL allocation provisioning failed; will retry" >&2
      fi
    fi

    case "$(resource_status)" in
      200)
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
