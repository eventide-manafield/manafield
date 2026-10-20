
                    set -eu
                    # Secret provisioning must never echo tokens to build logs.
                    set +x

                    modules_network="$(cat ci-plan/modules-network.txt)"
                    edge_network="$(cat ci-plan/edge-network.txt)"

                    postgres_count="$(
                      awk -F '\t' '$2 == "postgresql" { count += 1 } END { print count + 0 }' \
                        ci-plan/resources.tsv
                    )"

                    if [ "$postgres_count" -gt 1 ]; then
                      echo "PostgreSQL Provider v0 currently supports one Resource per Instance." >&2
                      exit 1
                    fi

                    postgres_resource_id=""
                    postgres_provider_image=""
                    compose_profiles=""

                    if [ "$postgres_count" -eq 1 ]; then
                      postgres_resource_id="$(
                        awk -F '\t' '$2 == "postgresql" { print $1; exit }' ci-plan/resources.tsv
                      )"
                      postgres_provider_image="$(
                        awk -F '\t' '$2 == "postgresql" { print $3; exit }' resource-providers.tsv
                      )"

                      test -n "$postgres_resource_id"
                      test -n "$postgres_provider_image"
                      compose_profiles="postgresql"
                    fi

                    mkdir -p "$RELEASE_DIR/modules"

                    : > "$RELEASE_DIR/postgresql-allocations.tsv"

                    if [ "$postgres_count" -eq 1 ]; then
                      mkdir -p "$INSTANCE_ROOT/secrets/postgresql"
                      chmod 0700 "$INSTANCE_ROOT/secrets"                         "$INSTANCE_ROOT/secrets/postgresql"                         2>/dev/null || true
                    fi

                    if [ -s postgresql-bindings.tsv ]; then
                      tab="$(printf '\t')"

                      while IFS="$tab" read -r module_id binding_slot binding_target binding_host binding_port database schema username secret_file; do
                        [ -n "$module_id" ] || continue

                        secret_name="$(basename "$secret_file")"
                        provider_secret_file="/run/manafield/postgresql/secrets/$binding_target/$secret_name"

                        printf '%s\t%s\t%s\t%s\t%s\n' \
                          "$binding_target" \
                          "$database" \
                          "$schema" \
                          "$username" \
                          "$provider_secret_file" \
                          >> "$RELEASE_DIR/postgresql-allocations.tsv"
                      done < postgresql-bindings.tsv
                    fi

                    cp deploy/compose.yml "$RELEASE_DIR/compose.yml"
                    cp build-plan.json "$RELEASE_DIR/build-plan.json"
                    ./manafield-cli bindings export \
                      --plan "$RELEASE_DIR/build-plan.json" \
                      --output "$RELEASE_DIR/manage-bindings.json"
                    cp resolved-images.env "$RELEASE_DIR/resolved-images.env"

                    printf '%s\n' \
                      "MANAFIELD_CORE_IMAGE=$CORE_IMAGE" \
                      "MANAFIELD_MODULES_PATH=$RELEASE_DIR/modules" \
                      "MANAFIELD_MODULES_NETWORK=$modules_network" \
                      "MANAFIELD_EDGE_NETWORK=$edge_network" \
                      "MANAFIELD_POSTGRES_PROVIDER_IMAGE=$postgres_provider_image" \
                      "MANAFIELD_POSTGRES_RESOURCE_ID=$postgres_resource_id" \
                      "MANAFIELD_POSTGRES_RESOURCE_NAME=Manafield PostgreSQL" \
                      "MANAFIELD_POSTGRES_DB=manafield" \
                      "MANAFIELD_POSTGRES_ALLOCATIONS_FILE_HOST=$RELEASE_DIR/postgresql-allocations.tsv" \
                      "MANAFIELD_POSTGRES_SECRETS_PATH=$INSTANCE_ROOT/secrets/postgresql" \
                      > "$RELEASE_DIR/release.env"

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

                    printf '%s\n' "$compose_profiles" > "$RELEASE_DIR/compose-profiles.txt"

                    cat > "$RELEASE_DIR/modules.compose.yml" <<'EOF'
services:
EOF

                    tab="$(printf '\t')"

                    while IFS="$tab" read -r module_id safe_id module_dir revision image build_type build_context dockerfile; do
                      manifest="$module_dir/manafield.module.json"
                      destination="$RELEASE_DIR/modules/$safe_id"

                      test -f "$manifest"
                      mkdir -p "$destination"
                      cp "$manifest" "$destination/manafield.module.json"

                      exposure_type="$(
                        awk -F '\t' -v id="$module_id" '$1 == id { print $2; exit }' \
                          ci-plan/module-exposures.tsv
                      )"
                      exposure_prefix="$(
                        awk -F '\t' -v id="$module_id" '$1 == id { print $4; exit }' \
                          ci-plan/module-exposures.tsv
                      )"
                      target_port="$(
                        awk -F '\t' -v id="$module_id" '$1 == id { print $5; exit }' \
                          ci-plan/module-exposures.tsv
                      )"

                      [ -n "$target_port" ] || target_port="8080"

                      base_path="/"
                      if [ "$exposure_type" = "prefix" ]; then
                        base_path="$exposure_prefix"
                      fi

                      service_id="module-$safe_id"

                      cat >> "$RELEASE_DIR/modules.compose.yml" <<EOF
  $service_id:
    image: $image
    restart: unless-stopped
    init: true
    environment:
      PORT: "$target_port"
      MANAFIELD_CORE_URL: http://core:8080
      MANAFIELD_MODULE_ID: "$module_id"
      MANAFIELD_WEB_BASE_PATH: "$base_path"
EOF

                      binding_mounts="$RELEASE_DIR/.binding-mounts-$safe_id"
                      : > "$binding_mounts"

                      if [ "$module_id" = "manafield-manage-web" ]; then
                        # Read-only directory mount: atomic file replacement is visible
                        # to a running container; a bind-mounted single file is not.
                        manage_assets="$INSTANCE_ROOT/manage-assets"
                        mkdir -p "$manage_assets"
                        chmod 0755 "$manage_assets"
                        cat >> "$RELEASE_DIR/modules.compose.yml" <<EOF
      MANAFIELD_MANAGE_BINDINGS_FILE: "/run/manafield/manage/bindings.json"
      MANAFIELD_MANAGE_CUSTOM_CSS_FILE: "/run/manafield/manage/custom.css"
EOF
                        printf '%s\t%s\n' \
                          "$manage_assets" "/run/manafield/manage" \
                          >> "$binding_mounts"
                      fi

                      if [ "$module_id" = "manafield-manage-web" ]; then
                        cat >> "$RELEASE_DIR/modules.compose.yml" <<EOF
      MANAFIELD_MANAGE_SSO_AUTHORIZATION_URL: "https://manafield.studio/account/oauth/authorize"
      MANAFIELD_MANAGE_SSO_TOKEN_URL: "http://module-manafield-account-core:8080/account/oauth/token"
      MANAFIELD_MANAGE_SSO_USERINFO_URL: "http://module-manafield-account-core:8080/account/oauth/userinfo"
      MANAFIELD_MANAGE_SSO_END_SESSION_URL: "http://module-manafield-account-core:8080/account/oauth/end-session"
      MANAFIELD_MANAGE_SSO_CLIENT_ID: "manafield-manage-web"
      MANAFIELD_MANAGE_SSO_REDIRECT_URL: "https://manage.manafield.studio/auth/callback"
EOF
                      fi

                      if [ "$module_id" = "manafield-account-core" ]; then
                        management_dir="$INSTANCE_ROOT/secrets/account-core"
                        management_file="$management_dir/management.token"
                        mkdir -p "$management_dir"
                        chmod 0700 "$INSTANCE_ROOT/secrets" "$management_dir"

                        if [ ! -f "$management_file" ]; then
                          umask 077
                          od -An -N32 -tx1 /dev/urandom | tr -d ' \\n' > "$management_file"
                        fi
                        test "$(wc -c < "$management_file")" -eq 64
                        chmod 0444 "$management_file"

                        printf '      MANAFIELD_ACCOUNT_MANAGEMENT_TOKEN_FILE: "/run/manafield/management/token"\\n' \
                          >> "$RELEASE_DIR/modules.compose.yml"
                        cat >> "$RELEASE_DIR/modules.compose.yml" <<'EOF'
      MANAFIELD_ACCOUNT_OAUTH_CLIENTS_JSON: '{"manafield-manage-web":"https://manage.manafield.studio/auth/callback"}'
EOF

                        printf '%s\\t%s\\n' \
                          "$management_file" "/run/manafield/management/token" \
                          >> "$binding_mounts"
                      fi

                      if [ "$module_id" = "manafield-account-role" ]; then
                        management_dir="$INSTANCE_ROOT/secrets/account-role"
                        management_file="$management_dir/management.token"
                        mkdir -p "$management_dir"
                        chmod 0700 "$INSTANCE_ROOT/secrets" "$management_dir"

                        if [ ! -f "$management_file" ]; then
                          umask 077
                          od -An -N32 -tx1 /dev/urandom | tr -d ' \\n' > "$management_file"
                        fi
                        test "$(wc -c < "$management_file")" -eq 64
                        chmod 0444 "$management_file"

                        printf '      MANAFIELD_ROLE_MANAGEMENT_TOKEN_FILE: "/run/manafield/management/token"\\n' \
                          >> "$RELEASE_DIR/modules.compose.yml"
                        printf '%s\\t%s\\n' \
                          "$management_file" "/run/manafield/management/token" \
                          >> "$binding_mounts"
                      fi

                      while IFS="$tab" read -r binding_module binding_slot binding_target; do
                        [ -n "$binding_module" ] || continue
                        [ "$binding_module" = "$module_id" ] || continue

                        binding_slot_upper="$(
                          printf '%s' "$binding_slot" | tr '[:lower:]' '[:upper:]'
                        )"

                        printf '      MANAFIELD_BINDING_%s_TARGET: "%s"\n' \
                          "$binding_slot_upper" \
                          "$binding_target" \
                          >> "$RELEASE_DIR/modules.compose.yml"

                        binding_host="$(
                          awk -F '\t' -v module="$module_id" -v slot="$binding_slot" \
                            '$1 == module && $2 == slot { print $4; exit }' \
                            postgresql-bindings.tsv
                        )"

                        if [ -n "$binding_host" ]; then
                          binding_port="$(
                            awk -F '\t' -v module="$module_id" -v slot="$binding_slot" \
                              '$1 == module && $2 == slot { print $5; exit }' \
                              postgresql-bindings.tsv
                          )"
                          binding_database="$(
                            awk -F '\t' -v module="$module_id" -v slot="$binding_slot" \
                              '$1 == module && $2 == slot { print $6; exit }' \
                              postgresql-bindings.tsv
                          )"
                          binding_schema="$(
                            awk -F '\t' -v module="$module_id" -v slot="$binding_slot" \
                              '$1 == module && $2 == slot { print $7; exit }' \
                              postgresql-bindings.tsv
                          )"
                          binding_username="$(
                            awk -F '\t' -v module="$module_id" -v slot="$binding_slot" \
                              '$1 == module && $2 == slot { print $8; exit }' \
                              postgresql-bindings.tsv
                          )"
                          binding_secret_file="$(
                            awk -F '\t' -v module="$module_id" -v slot="$binding_slot" \
                              '$1 == module && $2 == slot { print $9; exit }' \
                              postgresql-bindings.tsv
                          )"
                          binding_secret_target="/run/manafield/bindings/$binding_slot/password"

                          printf '      MANAFIELD_BINDING_%s_ENDPOINT_HOST: "%s"\n' \
                            "$binding_slot_upper" "$binding_host" \
                            >> "$RELEASE_DIR/modules.compose.yml"
                          printf '      MANAFIELD_BINDING_%s_ENDPOINT_PORT: "%s"\n' \
                            "$binding_slot_upper" "$binding_port" \
                            >> "$RELEASE_DIR/modules.compose.yml"
                          printf '      MANAFIELD_BINDING_%s_CONFIG_DATABASE: "%s"\n' \
                            "$binding_slot_upper" "$binding_database" \
                            >> "$RELEASE_DIR/modules.compose.yml"
                          printf '      MANAFIELD_BINDING_%s_CONFIG_SCHEMA: "%s"\n' \
                            "$binding_slot_upper" "$binding_schema" \
                            >> "$RELEASE_DIR/modules.compose.yml"
                          printf '      MANAFIELD_BINDING_%s_CONFIG_USERNAME: "%s"\n' \
                            "$binding_slot_upper" "$binding_username" \
                            >> "$RELEASE_DIR/modules.compose.yml"
                          printf '      MANAFIELD_BINDING_%s_SECRET_PASSWORD_FILE: "%s"\n' \
                            "$binding_slot_upper" "$binding_secret_target" \
                            >> "$RELEASE_DIR/modules.compose.yml"

                          printf '%s\t%s\n' \
                            "$binding_secret_file" \
                            "$binding_secret_target" \
                            >> "$binding_mounts"
                        fi
                      done < ci-plan/module-bindings.tsv

                      if [ -s "$binding_mounts" ]; then
                        cat >> "$RELEASE_DIR/modules.compose.yml" <<EOF
    volumes:
EOF
                        while IFS="$tab" read -r binding_secret_file binding_secret_target; do
                          cat >> "$RELEASE_DIR/modules.compose.yml" <<EOF
      - type: bind
        source: $binding_secret_file
        target: $binding_secret_target
        read_only: true
EOF
                        done < "$binding_mounts"
                      fi

                      rm -f "$binding_mounts"

                      cat >> "$RELEASE_DIR/modules.compose.yml" <<EOF
    depends_on:
      core:
        condition: service_healthy
    read_only: true
    tmpfs:
      - /tmp
    cap_drop:
      - ALL
    security_opt:
      - no-new-privileges:true
    networks:
      modules: {}
EOF

                      if [ -n "$exposure_type" ]; then
                        cat >> "$RELEASE_DIR/modules.compose.yml" <<EOF
      edge: {}
EOF
                      fi
                    done < module-sources.tsv
                