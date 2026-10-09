pipeline {
    agent any

    options {
        skipDefaultCheckout(true)
        disableConcurrentBuilds()
        timestamps()
        buildDiscarder(logRotator(numToKeepStr: "20"))
    }

    parameters {
        string(
            name: "INSTANCE_ROOT",
            defaultValue: "/opt/manafield/instance",
            description: "Private Manafield instance root containing instance.yaml"
        )
        string(
            name: "LOCAL_MODULES_ROOT",
            defaultValue: "/opt/manafield/local-modules",
            description: "Optional root containing local directory Modules"
        )
    }

    environment {
        DOCKER_BUILDKIT = "1"
        COMPOSE_DOCKER_CLI_BUILD = "1"
    }

    stages {
        stage("Checkout Core") {
            steps {
                checkout scm

                script {
                    env.CORE_SHA = sh(
                        script: "git rev-parse --short=12 HEAD",
                        returnStdout: true
                    ).trim()

                    env.CORE_IMAGE = "manafield-core:${env.CORE_SHA}"
                }
            }
        }

        stage("Prepare Instance") {
            steps {
                script {
                    env.INSTANCE_ROOT = params.INSTANCE_ROOT?.trim() ?: "/opt/manafield/instance"
                    env.LOCAL_MODULES_ROOT = params.LOCAL_MODULES_ROOT?.trim() ?: "/opt/manafield/local-modules"

                    echo "INSTANCE_ROOT=${env.INSTANCE_ROOT}"
                    echo "LOCAL_MODULES_ROOT=${env.LOCAL_MODULES_ROOT}"
                }

                sh '''
                    set -eu

                    if [ ! -d "$INSTANCE_ROOT" ]; then
                      echo "Manafield Instance root is not visible to Jenkins:" >&2
                      echo "  $INSTANCE_ROOT" >&2
                      echo >&2
                      echo "If Jenkins runs in a container, bind-mount INSTANCE_ROOT into the container at the same path." >&2
                      exit 1
                    fi
                '''
            }
        }

        stage("Bootstrap Instance") {
            steps {
                script {
                    def instanceExists = sh(
                        script: 'test -f "$INSTANCE_ROOT/instance.yaml"',
                        returnStatus: true
                    ) == 0

                    if (instanceExists) {
                        echo "Using existing Instance Definition: ${env.INSTANCE_ROOT}/instance.yaml"
                    } else {
                        def bootstrap = input(
                            message: "No instance.yaml was found. Configure the first Manafield instance.",
                            ok: "Create Instance",
                            parameters: [
                                booleanParam(
                                    name: "INSTALL_POSTGRES",
                                    defaultValue: false,
                                    description: "Install the Manafield PostgreSQL Resource"
                                ),
                                booleanParam(
                                    name: "INSTALL_EXAMPLE_WEB",
                                    defaultValue: false,
                                    description: "Install the Example Web Module (planned; not implemented yet)"
                                ),
                                booleanParam(
                                    name: "INSTALL_EXAMPLE_ACCOUNT",
                                    defaultValue: false,
                                    description: "Install the Example Account Module (planned; implies PostgreSQL + Example Web)"
                                ),
                                string(
                                    name: "ADMIN_USERNAME",
                                    defaultValue: "admin",
                                    description: "Initial example administrator username"
                                ),
                                string(
                                    name: "ADMIN_PASSWORD",
                                    defaultValue: "admin",
                                    description: "Initial example administrator password"
                                )
                            ]
                        )

                        def installPostgres = bootstrap["INSTALL_POSTGRES"] as boolean
                        def installWeb = bootstrap["INSTALL_EXAMPLE_WEB"] as boolean
                        def installAccount = bootstrap["INSTALL_EXAMPLE_ACCOUNT"] as boolean

                        if (installAccount) {
                            installPostgres = true
                            installWeb = true
                        }

                        if (installWeb || installAccount) {
                            def selected = []
                            if (installWeb) {
                                selected << "Example Web"
                            }
                            if (installAccount) {
                                selected << "Example Account"
                            }

                            error(
                                "Selected bootstrap examples are not implemented yet: " +
                                selected.join(", ") +
                                ". PostgreSQL can already be bootstrapped independently."
                            )
                        }

                        env.BOOTSTRAP_INSTALL_POSTGRES = installPostgres ? "true" : "false"

                        sh '''
                            set -eu
                            cp deploy/instance.bootstrap.yaml "$INSTANCE_ROOT/instance.yaml"

                            if [ "${BOOTSTRAP_INSTALL_POSTGRES:-false}" = "true" ]; then
                              cat >> "$INSTANCE_ROOT/instance.yaml" <<'EOF'

resources:
  - id: manafield-postgres
    enabled: true
    provider: postgresql
EOF
                            fi

                            echo "Created bootstrap Instance Definition:"
                            echo "  $INSTANCE_ROOT/instance.yaml"
                        '''
                    }
                }
            }
        }

        stage("Resolve Build Plan") {
            steps {
                sh '''
                    set -eu

                    rm -rf ci-plan build-plan.json manafield-cli

                    docker build \
                      --target core-runtime \
                      --tag "manafield-cli:$CORE_SHA" \
                      .

                    cli_container="$(docker create "manafield-cli:$CORE_SHA")"
                    trap 'docker rm -f "$cli_container" >/dev/null 2>&1 || true' EXIT

                    docker cp \
                      "$cli_container:/usr/local/bin/manafield" \
                      ./manafield-cli

                    chmod +x ./manafield-cli
                    ./manafield-cli plan \
                      "$INSTANCE_ROOT/instance.yaml" \
                      --output build-plan.json \
                      --ci-output ci-plan

                    test -s ci-plan/modules.tsv
                    grep -q '^manafield-reference	' ci-plan/modules.tsv
                '''
            }
        }

        stage("Discover Local Modules") {
            steps {
                sh '''
                    set -eu

                    : > local-modules.tsv

                    if ! awk -F '\t' '$2 == "dir" { found = 1 } END { exit(found ? 0 : 1) }' ci-plan/modules.tsv; then
                      echo "No local directory Modules requested."
                      exit 0
                    fi

                    if [ -z "${LOCAL_MODULES_ROOT:-}" ]; then
                      echo "LOCAL_MODULES_ROOT is required when a Module uses source.type=dir" >&2
                      exit 1
                    fi

                    if [ ! -d "$LOCAL_MODULES_ROOT" ]; then
                      echo "Local Module root is not visible to Jenkins:" >&2
                      echo "  $LOCAL_MODULES_ROOT" >&2
                      echo >&2
                      echo "If Jenkins runs in a container, bind-mount LOCAL_MODULES_ROOT into the container at the same path." >&2
                      exit 1
                    fi

                    local_root="$(cd "$LOCAL_MODULES_ROOT" && pwd -P)"

                    for module_dir in "$local_root"/*; do
                      [ -d "$module_dir" ] || continue
                      [ -f "$module_dir/manafield.module.json" ] || continue

                      module_id="$(basename "$module_dir")"

                      printf '%s\t%s\n' \
                        "$module_id" \
                        "$module_dir" \
                        >> local-modules.tsv
                    done

                    tab="$(printf '\t')"

                    while IFS="$tab" read -r module_id source_type source_value source_ref source_subdir build_type build_context dockerfile; do
                      [ -n "$module_id" ] || continue
                      [ "$source_type" = "dir" ] || continue

                      if ! awk -F '\t' -v id="$module_id" '$1 == id { found = 1 } END { exit(found ? 0 : 1) }' local-modules.tsv; then
                        echo "Local Module '$module_id' was not found under $local_root" >&2
                        exit 1
                      fi
                    done < ci-plan/modules.tsv

                    echo "Discovered local Modules:"
                    cat local-modules.tsv
                '''
            }
        }

        stage("Ingress Provider Preflight") {
            steps {
                script {
                    env.INGRESS_PROVIDER_STATE = sh(
                        script: '''
                            set -eu

                            if [ ! -s ci-plan/ingress-provider.txt ]; then
                              echo "NOT_CONFIGURED"
                              exit 0
                            fi

                            provider="$(cat ci-plan/ingress-provider.txt)"

                            case "$provider" in
                              traefik)
                                test -s ci-plan/ingress-bootstrap-compose.txt
                                test -s ci-plan/ingress-bootstrap-service.txt
                                test -s ci-plan/ingress-output.txt

                                compose_file="$(cat ci-plan/ingress-bootstrap-compose.txt)"
                                service="$(cat ci-plan/ingress-bootstrap-service.txt)"
                                output="$(cat ci-plan/ingress-output.txt)"
                                dynamic_dir="$(dirname "$output")"

                                set +e
                                bash deploy/ingress/traefik-provider.sh \
                                  check \
                                  "$compose_file" \
                                  "$service" \
                                  "$dynamic_dir" \
                                  >/tmp/manafield-ingress-check.log 2>&1
                                status=$?
                                set -e

                                case "$status" in
                                  0)
                                    echo "READY"
                                    ;;
                                  2)
                                    echo "BOOTSTRAP_REQUIRED"
                                    ;;
                                  *)
                                    cat /tmp/manafield-ingress-check.log >&2
                                    exit "$status"
                                    ;;
                                esac
                                ;;
                              *)
                                echo "Unsupported ingress provider: $provider" >&2
                                exit 1
                                ;;
                            esac
                        ''',
                        returnStdout: true
                    ).trim()

                    if (env.INGRESS_PROVIDER_STATE == "BOOTSTRAP_REQUIRED") {
                        input(
                            message: "Traefik ingress provider bootstrap is required for this Manafield instance.",
                            ok: "Bootstrap Traefik"
                        )

                        sh '''
                            set -eu

                            compose_file="$(cat ci-plan/ingress-bootstrap-compose.txt)"
                            service="$(cat ci-plan/ingress-bootstrap-service.txt)"
                            output="$(cat ci-plan/ingress-output.txt)"
                            dynamic_dir="$(dirname "$output")"

                            bash deploy/ingress/traefik-provider.sh \
                              apply \
                              "$compose_file" \
                              "$service" \
                              "$dynamic_dir"
                        '''
                    } else {
                        echo "Ingress provider state: ${env.INGRESS_PROVIDER_STATE}"
                    }
                }
            }
        }

        stage("Verify Core") {
            steps {
                sh '''
                    set -eu

                    docker build \
                      --target verify \
                      --tag "manafield-core-verify:$CORE_SHA" \
                      .
                '''
            }
        }

        stage("Checkout Modules") {
            steps {
                sh '''
                    set -eu

                    rm -rf modules
                    mkdir -p modules
                    : > module-sources.tsv

                    tab="$(printf '\t')"

                    while IFS="$tab" read -r module_id source_type source_value source_ref source_subdir build_type build_context dockerfile; do
                      [ -n "$module_id" ] || continue

                      safe_id="$(
                        printf '%s' "$module_id" \
                          | tr '[:upper:]' '[:lower:]' \
                          | sed 's/[^a-z0-9_.-]/-/g'
                      )"

                      module_dir="$WORKSPACE/modules/$safe_id"

                      case "$source_type" in
                        git)
                          repo_dir="$WORKSPACE/modules/$safe_id-source"

                          git clone --no-checkout "$source_value" "$repo_dir"
                          git -C "$repo_dir" fetch --depth=1 origin "$source_ref"
                          git -C "$repo_dir" checkout --detach FETCH_HEAD
                          revision="$(git -C "$repo_dir" rev-parse --short=12 HEAD)"

                          module_dir="$repo_dir"
                          if [ "$source_subdir" != "." ]; then
                            module_dir="$repo_dir/$source_subdir"
                          fi

                          test -d "$module_dir" || {
                            echo "Git Module '$module_id' subdir '$source_subdir' does not exist." >&2
                            exit 1
                          }
                          ;;
                        dir)
                          source_dir="$(
                            awk -F '\t' -v id="$module_id" \
                              '$1 == id { print $2; exit }' \
                              local-modules.tsv
                          )"

                          test -n "$source_dir"
                          cp -a "$source_dir" "$module_dir"

                          revision="dir-$(
                            find "$module_dir" \
                              -type f \
                              -not -path '*/.git/*' \
                              -print0 \
                            | sort -z \
                            | xargs -0 sha256sum \
                            | sha256sum \
                            | cut -c1-12
                          )"
                          ;;
                        *)
                          echo "Unsupported Module source type: $source_type" >&2
                          exit 1
                          ;;
                      esac

                      test -f "$module_dir/manafield.module.json" || {
                        echo "Module '$module_id' manifest was not found at $module_dir/manafield.module.json" >&2
                        exit 1
                      }

                      image="$safe_id:$revision"

                      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
                        "$module_id" \
                        "$safe_id" \
                        "$module_dir" \
                        "$revision" \
                        "$image" \
                        "$build_type" \
                        "$build_context" \
                        "$dockerfile" \
                        >> module-sources.tsv
                    done < ci-plan/modules.tsv
                '''
            }
        }

        stage("Build Images") {
            steps {
                sh '''
                    set -eu
                    ./manafield-cli build "$WORKSPACE" "$CORE_SHA" "$CORE_IMAGE"
                '''
            }
        }

        stage("Materialize Resource Bindings") {
            steps {
                sh '''
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
                          password="$(od -An -N24 -tx1 /dev/urandom | tr -d ' \\n')"
                          test -n "$password"
                          printf '%s\\n' "$password" > "$secret_file"
                        fi
                        chmod 0444 "$secret_file"
                        printf '%s\\t%s\\t%s\\t%s\\t%s\\t%s\\t%s\\t%s\\t%s\\n' \
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
                '''
            }
        }

        stage("Stage Release") {
            steps {
                script {
                    env.RELEASE_DIR = "${env.INSTANCE_ROOT}/releases/${env.BUILD_NUMBER}"
                }

                sh '''
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
                '''
            }
        }

        stage("Render Ingress") {
            steps {
                sh '''
                    set -eu

                    if [ ! -s ci-plan/ingress-provider.txt ]; then
                      echo "No ingress provider configured."
                      exit 0
                    fi

                    provider="$(cat ci-plan/ingress-provider.txt)"

                    case "$provider" in
                      traefik)
                        docker build \
                          --target ingress-traefik-runtime \
                          --tag "manafield-ingress-traefik:$CORE_SHA" \
                          .

                        adapter_container="$(docker create "manafield-ingress-traefik:$CORE_SHA")"
                        trap 'docker rm -f "$adapter_container" >/dev/null 2>&1 || true' EXIT

                        docker cp \
                          "$adapter_container:/usr/local/bin/manafield-ingress-traefik" \
                          ./manafield-ingress-traefik

                        chmod +x ./manafield-ingress-traefik
                        ./manafield-ingress-traefik \
                          build-plan.json \
                          "$RELEASE_DIR/ingress-traefik.yml"

                        cp "$RELEASE_DIR/ingress-traefik.yml" ingress-traefik.yml
                        ;;
                      *)
                        echo "Unsupported ingress provider: $provider" >&2
                        exit 1
                        ;;
                    esac
                '''
            }
        }

        stage("Deploy") {
            steps {
                sh '''
                    set -eu

                    # The staged release is deployed by the shared CLI executor.
                    test -x ./manafield-cli
                    ./manafield-cli deploy "$RELEASE_DIR"
                '''
            }
        }

        stage("Publish Ingress") {
            steps {
                sh '''
                    set -eu

                    if [ ! -s ci-plan/ingress-provider.txt ]; then
                      echo "No ingress provider configured."
                      exit 0
                    fi

                    provider="$(cat ci-plan/ingress-provider.txt)"
                    output="$(cat ci-plan/ingress-output.txt)"

                    case "$provider" in
                      traefik)
                        test -s "$RELEASE_DIR/ingress-traefik.yml"

                        mkdir -p "$(dirname "$output")"
                        temporary="$output.tmp.$BUILD_NUMBER"

                        cp "$RELEASE_DIR/ingress-traefik.yml" "$temporary"
                        chmod 0644 "$temporary"
                        mv -f "$temporary" "$output"

                        echo "Published Traefik ingress config: $output"
                        ;;
                      *)
                        echo "Unsupported ingress provider: $provider" >&2
                        exit 1
                        ;;
                    esac
                '''
            }
        }

        stage("Verify Deployment") {
            steps {
                sh '''
                    set -eu

                    compose_profiles="$(cat "$RELEASE_DIR/compose-profiles.txt")"
                    if [ -n "$compose_profiles" ]; then
                      export COMPOSE_PROFILES="$compose_profiles"
                    else
                      unset COMPOSE_PROFILES || true
                    fi

                    compose() {
                      docker compose \
                        --env-file "$RELEASE_DIR/release.env" \
                        --file "$RELEASE_DIR/compose.yml" \
                        --file "$RELEASE_DIR/modules.compose.yml" \
                        "$@"
                    }

                    core_container="$(compose ps -q core)"
                    test -n "$core_container"

                    verify_expected_modules() {
                      docker exec "$core_container" \
                        curl --fail --silent --show-error \
                        http://127.0.0.1:8080/modules >/tmp/manafield-modules.json \
                        || return 1

                      tab="$(printf '\t')"
                      while IFS="$tab" read -r module_id rest; do
                        [ -n "$module_id" ] || continue

                        if ! docker exec "$core_container" \
                          curl --fail --silent --show-error \
                          "http://127.0.0.1:8080/modules/$module_id" \
                          > /dev/null; then
                          echo "Expected module is not registered: $module_id" >&2
                          return 1
                        fi
                      done < ci-plan/modules.tsv
                    }

                    verify_expected_modules

                    # Publish the active binding projection only after Module Registry
                    # verification succeeds. Never publish candidate/failed Releases.
                    if awk -F '\t' '$1 == "manafield-manage-web" { found=1 } END { exit(found ? 0 : 1) }' ci-plan/modules.tsv; then
                      target_dir="$INSTANCE_ROOT/manage-assets"
                      test -d "$target_dir"
                      test -s "$RELEASE_DIR/manage-bindings.json"
                      tmp="$target_dir/.bindings.json.$BUILD_NUMBER.tmp"
                      cp "$RELEASE_DIR/manage-bindings.json" "$tmp"
                      chmod 0644 "$tmp"
                      mv -f "$tmp" "$target_dir/bindings.json"
                      echo "Published verified Manage binding snapshot."
                    fi
                    echo "Deployment verification passed."
                '''
            }
        }
    }
}
