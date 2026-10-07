#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: traefik-provider.sh <check|apply> <compose-file> <service> <dynamic-dir>" >&2
  exit 64
}

[ "$#" -eq 4 ] || usage

action="$1"
compose_file="$2"
service="$3"
dynamic_dir="$4"

[ -f "$compose_file" ] || {
  echo "Traefik Compose file not found: $compose_file" >&2
  exit 1
}

compose_dir="$(cd "$(dirname "$compose_file")" && pwd)"
compose_file="$(cd "$(dirname "$compose_file")" && pwd)/$(basename "$compose_file")"
dynamic_dir="$(readlink -m "$dynamic_dir")"

check_provider() {
  rendered="$(cd "$compose_dir" && docker compose -f "$compose_file" config)"

  printf '%s\n' "$rendered" \
    | grep -F -- '--providers.file.directory=/etc/traefik/dynamic' >/dev/null \
    || return 1

  printf '%s\n' "$rendered" \
    | grep -F -- '--providers.file.watch=true' >/dev/null \
    || return 1

  printf '%s\n' "$rendered" \
    | grep -F "source: $dynamic_dir" >/dev/null \
    || return 1

  printf '%s\n' "$rendered" \
    | grep -F 'target: /etc/traefik/dynamic' >/dev/null \
    || return 1

  container="$(cd "$compose_dir" && docker compose -f "$compose_file" ps -q "$service")"
  [ -n "$container" ] || return 1
  [ "$(docker inspect -f '{{.State.Running}}' "$container")" = "true" ] || return 1

  return 0
}

case "$action" in
  check)
    if check_provider; then
      echo "Traefik ingress provider is ready."
      exit 0
    fi

    echo "Traefik ingress provider bootstrap required." >&2
    exit 2
    ;;

  apply)
    mkdir -p "$dynamic_dir"

    stamp="$(date +%Y%m%d-%H%M%S)"
    backup="$compose_file.bak.$stamp"

    echo "== Backup Traefik Compose =="
    cp -a "$compose_file" "$backup"
    echo "$backup"

    echo
    echo "== Configure Traefik file provider =="

    if ! grep -Fq -- '--providers.file.directory=/etc/traefik/dynamic' "$compose_file"; then
      grep -Fq -- '      - --providers.docker.exposedbydefault=false' "$compose_file" || {
        echo "Traefik command insertion point not found" >&2
        exit 1
      }

      temporary="$(mktemp "$compose_file.tmp.XXXXXX")"
      awk '
        { print }
        $0 == "      - --providers.docker.exposedbydefault=false" {
          print "      - --providers.file.directory=/etc/traefik/dynamic"
          print "      - --providers.file.watch=true"
        }
      ' "$compose_file" > "$temporary"

      cat "$temporary" > "$compose_file"
      rm -f "$temporary"
    fi

    if ! grep -Fq -- ':/etc/traefik/dynamic' "$compose_file"; then
      grep -Fq -- '      - ./letsencrypt:/letsencrypt' "$compose_file" || {
        echo "Traefik volume insertion point not found" >&2
        exit 1
      }

      temporary="$(mktemp "$compose_file.tmp.XXXXXX")"
      awk -v dynamic_dir="$dynamic_dir" '
        { print }
        $0 == "      - ./letsencrypt:/letsencrypt" {
          print "      - " dynamic_dir ":/etc/traefik/dynamic:ro"
        }
      ' "$compose_file" > "$temporary"

      cat "$temporary" > "$compose_file"
      rm -f "$temporary"
    fi

    echo
    echo "== Validate Compose =="

    (
      cd "$compose_dir"
      docker compose -f "$compose_file" config >/dev/null
    )

    echo "Compose validation passed."

    echo
    echo "== Recreate Traefik provider service =="

    (
      cd "$compose_dir"
      docker compose -f "$compose_file" up -d --force-recreate "$service"
    )

    attempts=30
    while [ "$attempts" -gt 0 ]; do
      if check_provider; then
        echo "Traefik ingress provider bootstrap complete."
        exit 0
      fi

      attempts=$((attempts - 1))
      sleep 2
    done

    echo "Traefik ingress provider did not become ready." >&2
    echo "Backup: $backup" >&2
    exit 1
    ;;

  *)
    usage
    ;;
esac
