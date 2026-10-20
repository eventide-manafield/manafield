#!/bin/sh
set -eu
                    # Bridge the declared Core loggingState Resource binding into
                    # a private DSN file. No credentials enter public Build Plans
                    # or Jenkins logs; the Postgres Provider owns schema allocation.
                    if [ -s ci-plan/core-bindings.tsv ]; then
                      tab="$(printf '\t')"
                      while IFS="$tab" read -r consumer slot target host port database schema username secret_file; do
                        [ "$consumer" = "manafield-core" ] || continue
                        [ "$slot" = "loggingState" ] || exit 1
                        test -f "$secret_file"
                        password="$(head -n 1 "$secret_file")"
                        test "${#password}" -eq 48
                        case "$password" in *[!a-f0-9]*|'')
                          echo "Invalid generated Core logging database credential." >&2
                          exit 1
                          ;; esac

                        dsn_dir="$INSTANCE_ROOT/secrets/core-logging"
                        mkdir -p "$dsn_dir"
                        chmod 0700 "$dsn_dir"
                        dsn_file="$dsn_dir/postgres.dsn"
                        temporary="$dsn_dir/.postgres.dsn.$BUILD_NUMBER.tmp"
                        umask 077
                        printf 'postgres://%s:%s@%s:%s/%s?sslmode=disable\n' \
                          "$username" "$password" "$host" "$port" "$database" \
                          > "$temporary"
                        chmod 0444 "$temporary"
                        mv -f "$temporary" "$dsn_file"

                        printf '%s\n' \
                          "MANAFIELD_LOG_POSTGRES_DSN_FILE=/run/manafield/logging/postgres.dsn" \
                          "MANAFIELD_LOG_POSTGRES_SCHEMA=$schema" \
                          "MANAFIELD_LOG_DSN_FILE_HOST=$dsn_file" \
                          >> "$RELEASE_DIR/release.env"
                      done < postgresql-bindings.tsv
                    fi

