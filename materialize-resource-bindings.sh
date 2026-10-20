
                    # Never print credentials or their generation commands to Jenkins logs.
                    set +x
                    set -eu

                    : > postgresql-bindings.tsv

                    if [ ! -s ci-plan/module-bindings.tsv ] && [ ! -s ci-plan/core-bindings.tsv ]; then
                      echo "No Resource bindings require materialization."
                      exit 0
                    fi

                    tab="$(printf '\t')"

                    postgres_identifier() {
                      raw="$1"
                      normalized="$(
                        printf '%s' "$raw" \
                          | tr '[:upper:].-' '[:lower:]__' \
                          | sed 's/[^a-z0-9_]/_/g'
                      )"

                      if [ "${#normalized}" -gt 55 ]; then
                        digest="$(printf '%s' "$normalized" | sha256sum | cut -c1-8)"
                        normalized="$(printf '%s' "$normalized" | cut -c1-46)_$digest"
                      fi

                      printf '%s' "$normalized"
                    }

                    while IFS="$tab" read -r module_id binding_slot binding_target; do
                      [ -n "$module_id" ] || continue

                      provider="$(
                        awk -F '\t' -v id="$binding_target" \
                          '$1 == id { print $2; exit }' \
                          ci-plan/resources.tsv
                      )"

                      # Module-to-Module bindings do not need Resource materialization.
                      [ -n "$provider" ] || continue

                      case "$provider" in
                        postgresql)
                          safe_module="$(
                            awk -F '\t' -v id="$module_id" \
                              '$1 == id { print $2; exit }' \
                              module-sources.tsv
                          )"
                          test -n "$safe_module"

                          identifier="$(postgres_identifier "mf_${safe_module}_${binding_slot}")"
                          database="manafield"
                          schema="$identifier"
                          username="$identifier"

                          secret_dir="$INSTANCE_ROOT/secrets/postgresql/$binding_target"
                          secret_file="$secret_dir/$identifier.password"

                          mkdir -p "$secret_dir"
                          chmod 0700 "$INSTANCE_ROOT/secrets" \
                            "$INSTANCE_ROOT/secrets/postgresql" \
                            "$secret_dir" \
                            2>/dev/null || true

                          if [ ! -f "$secret_file" ]; then
                            umask 077
                            password="$(
                              od -An -N24 -tx1 /dev/urandom | tr -d ' \n'
                            )"
                            test -n "$password"
                            printf '%s\n' "$password" > "$secret_file"
                          fi

                          # Parent directories are 0700 on the host. The file itself is
                          # read-only so non-root Module containers can consume the bind mount.
                          chmod 0444 "$secret_file"

                          printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
                            "$module_id" \
                            "$binding_slot" \
                            "$binding_target" \
                            "manafield-postgres" \
                            "5432" \
                            "$database" \
                            "$schema" \
                            "$username" \
                            "$secret_file" \
                            >> postgresql-bindings.tsv
                          ;;
                        *)
                          echo "No connection materializer for Resource provider '$provider' (binding $module_id.$binding_slot -> $binding_target)"
                          ;;
                      esac
                    done < ci-plan/module-bindings.tsv

                    # Core is a first-class Resource consumer for logging, but
                    # has no Module manifest or module-sources.tsv entry.
                    if [ -s ci-plan/core-bindings.tsv ]; then
                      while IFS="$tab" read -r binding_slot binding_target; do
                        [ "$binding_slot" = "loggingState" ] || {
                          echo "Unknown Core binding slot '$binding_slot'." >&2
                          exit 1
                        }
                        provider="$(
                          awk -F '\t' -v id="$binding_target" \
                            '$1 == id { print $2; exit }' ci-plan/resources.tsv
                        )"
                        [ "$provider" = "postgresql" ] || {
                          echo "Core logging requires a PostgreSQL Resource." >&2
                          exit 1
                        }

                        identifier="$(postgres_identifier "mf_manafield_core_logging")"
                        secret_dir="$INSTANCE_ROOT/secrets/postgresql/$binding_target"
                        secret_file="$secret_dir/$identifier.password"
                        mkdir -p "$secret_dir"
                        chmod 0700 "$INSTANCE_ROOT/secrets" \
                          "$INSTANCE_ROOT/secrets/postgresql" "$secret_dir" \
                          2>/dev/null || true

                        if [ ! -f "$secret_file" ]; then
                          umask 077
                          password="$(od -An -N24 -tx1 /dev/urandom | tr -d ' \n')"
                          test -n "$password"
                          printf '%s\n' "$password" > "$secret_file"
                        fi
                        chmod 0444 "$secret_file"
                        printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
                          "manafield-core" "$binding_slot" "$binding_target" \
                          "manafield-postgres" "5432" "manafield" \
                          "$identifier" "$identifier" "$secret_file" \
                          >> postgresql-bindings.tsv
                      done < ci-plan/core-bindings.tsv
                    fi

                    if [ -s postgresql-bindings.tsv ]; then
                      echo "Materialized PostgreSQL bindings:"
                      awk -F '\t' '{ print "  " $1 "." $2 " -> " $3 " (database=" $6 " schema=" $7 " role=" $8 ")" }' \
                        postgresql-bindings.tsv
                    fi
                