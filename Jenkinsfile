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
            defaultValue: "/opt/manafield/ai-workspace/instance",
            description: "Private Manafield instance root containing instance.yaml"
        )
        string(
            name: "LOCAL_MODULES_ROOT",
            defaultValue: "/opt/manafield/ai-workspace/local-modules",
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

        stage("Confirm Instance Paths") {
            steps {
                script {
                    def confirmed = input(
                        message: "Use these Manafield paths? Change them if needed, then confirm.",
                        ok: "Build with these paths",
                        parameters: [
                            string(
                                name: "INSTANCE_ROOT",
                                defaultValue: params.INSTANCE_ROOT?.trim() ?: "/opt/manafield/ai-workspace/instance",
                                description: "Directory containing the private instance.yaml"
                            ),
                            string(
                                name: "LOCAL_MODULES_ROOT",
                                defaultValue: params.LOCAL_MODULES_ROOT?.trim() ?: "/opt/manafield/ai-workspace/local-modules",
                                description: "Root scanned for local directory Modules"
                            )
                        ]
                    )

                    env.INSTANCE_ROOT = confirmed["INSTANCE_ROOT"]?.trim() ?: ""
                    env.LOCAL_MODULES_ROOT = confirmed["LOCAL_MODULES_ROOT"]?.trim() ?: ""

                    if (!env.INSTANCE_ROOT) {
                        error("INSTANCE_ROOT must not be empty")
                    }

                    echo "INSTANCE_ROOT=${env.INSTANCE_ROOT}"
                    echo "LOCAL_MODULES_ROOT=${env.LOCAL_MODULES_ROOT ?: '(unused)'}"
                }
            }
        }

        stage("Resolve Build Plan") {
            steps {
                sh '''
                    set -eu

                    if [ ! -f "$INSTANCE_ROOT/instance.yaml" ]; then
                      echo "Manafield Instance Definition is not visible to Jenkins:" >&2
                      echo "  $INSTANCE_ROOT/instance.yaml" >&2
                      echo >&2
                      echo "If Jenkins runs in a container, bind-mount INSTANCE_ROOT into the container at the same path." >&2
                      exit 1
                    fi

                    rm -rf ci-plan build-plan.json manafield-build-plan

                    docker build \
                      --target build-plan-runtime \
                      --tag "manafield-build-plan:$CORE_SHA" \
                      .

                    resolver_container="$(docker create "manafield-build-plan:$CORE_SHA")"
                    trap 'docker rm -f "$resolver_container" >/dev/null 2>&1 || true' EXIT

                    docker cp \
                      "$resolver_container:/usr/local/bin/manafield-build-plan" \
                      ./manafield-build-plan

                    chmod +x ./manafield-build-plan
                    ./manafield-build-plan \
                      "$INSTANCE_ROOT/instance.yaml" \
                      build-plan.json \
                      ci-plan

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

                    while IFS="$tab" read -r module_id source_type source_value source_ref build_type build_context dockerfile; do
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

                    while IFS="$tab" read -r module_id source_type source_value source_ref build_type build_context dockerfile; do
                      [ -n "$module_id" ] || continue

                      safe_id="$(
                        printf '%s' "$module_id" \
                          | tr '[:upper:]' '[:lower:]' \
                          | sed 's/[^a-z0-9_.-]/-/g'
                      )"

                      module_dir="$WORKSPACE/modules/$safe_id"

                      case "$source_type" in
                        git)
                          git clone --no-checkout "$source_value" "$module_dir"
                          git -C "$module_dir" fetch --depth=1 origin "$source_ref"
                          git -C "$module_dir" checkout --detach FETCH_HEAD
                          revision="$(git -C "$module_dir" rev-parse --short=12 HEAD)"
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

                    docker build \
                      --target core-runtime \
                      --label "org.opencontainers.image.revision=$CORE_SHA" \
                      --tag "$CORE_IMAGE" \
                      .

                    {
                      printf 'core.image=%s\n' "$CORE_IMAGE"
                      printf 'core.revision=%s\n' "$CORE_SHA"
                    } > resolved-images.env

                    tab="$(printf '\t')"

                    while IFS="$tab" read -r module_id safe_id module_dir revision image build_type build_context dockerfile; do
                      [ "$build_type" = "docker" ] || {
                        echo "Unsupported Module build type: $build_type" >&2
                        exit 1
                      }

                      docker build \
                        --label "org.opencontainers.image.revision=$revision" \
                        --tag "$image" \
                        --file "$module_dir/$dockerfile" \
                        "$module_dir/$build_context"

                      printf 'module.%s.image=%s\n' "$module_id" "$image" \
                        >> resolved-images.env
                      printf 'module.%s.revision=%s\n' "$module_id" "$revision" \
                        >> resolved-images.env
                    done < module-sources.tsv
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

                    modules_network="$(cat ci-plan/modules-network.txt)"
                    edge_network="$(cat ci-plan/edge-network.txt)"
                    reference_image="$(
                      awk -F '\t' '$1 == "manafield-reference" { print $5; exit }' module-sources.tsv
                    )"

                    test -n "$reference_image"

                    mkdir -p "$RELEASE_DIR/modules"

                    cp deploy/compose.yml "$RELEASE_DIR/compose.yml"
                    cp build-plan.json "$RELEASE_DIR/build-plan.json"
                    cp resolved-images.env "$RELEASE_DIR/resolved-images.env"

                    printf '%s\n' \
                      "MANAFIELD_CORE_IMAGE=$CORE_IMAGE" \
                      "MANAFIELD_REFERENCE_IMAGE=$reference_image" \
                      "MANAFIELD_MODULES_PATH=$RELEASE_DIR/modules" \
                      "MANAFIELD_MODULES_NETWORK=$modules_network" \
                      "MANAFIELD_EDGE_NETWORK=$edge_network" \
                      > "$RELEASE_DIR/release.env"

                    tab="$(printf '\t')"

                    while IFS="$tab" read -r module_id safe_id module_dir revision image build_type build_context dockerfile; do
                      manifest="$module_dir/manafield.module.json"
                      destination="$RELEASE_DIR/modules/$safe_id"

                      test -f "$manifest"
                      mkdir -p "$destination"
                      cp "$manifest" "$destination/manafield.module.json"
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

                    docker compose \
                      --env-file "$RELEASE_DIR/release.env" \
                      --file "$RELEASE_DIR/compose.yml" \
                      up -d --no-build --remove-orphans

                    docker compose \
                      --env-file "$RELEASE_DIR/release.env" \
                      --file "$RELEASE_DIR/compose.yml" \
                      ps
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

                    core_container="$(
                      docker compose \
                        --env-file "$RELEASE_DIR/release.env" \
                        --file "$RELEASE_DIR/compose.yml" \
                        ps -q core
                    )"

                    test -n "$core_container"

                    attempts=30
                    while [ "$attempts" -gt 0 ]; do
                      if docker exec "$core_container" \
                           curl --fail --silent --show-error \
                           http://127.0.0.1:8080/health >/tmp/manafield-core-health.json \
                        && docker exec "$core_container" \
                           curl --fail --silent --show-error \
                           http://127.0.0.1:8080/modules >/tmp/manafield-modules.json \
                        && grep -Fq '"id":"manafield-reference"' /tmp/manafield-modules.json \
                        && while IFS="$(printf '\t')" read -r module_id _; do
                             [ -n "$module_id" ] || continue
                             grep -Fq "\"id\":\"$module_id\"" /tmp/manafield-modules.json
                           done < module-sources.tsv \
                        && docker exec "$core_container" \
                           curl --fail --silent --show-error \
                           http://reference:8080/manafield/health >/tmp/manafield-reference-health.json \
                        && docker exec "$core_container" \
                           curl --fail --silent --show-error \
                           http://reference:8080/api/core/modules >/tmp/manafield-reference-modules.json \
                        && grep -Fq '"id":"manafield-reference"' /tmp/manafield-reference-modules.json
                      then
                        cat /tmp/manafield-core-health.json
                        echo
                        cat /tmp/manafield-reference-health.json
                        echo
                        break
                      fi

                      attempts=$((attempts - 1))

                      if [ "$attempts" -eq 0 ]; then
                        echo "Manafield deployment verification timed out." >&2
                        exit 1
                      fi

                      sleep 2
                    done

                    ln -sfn "releases/$BUILD_NUMBER" "$INSTANCE_ROOT/current"
                '''
            }
        }
    }

    post {
        always {
            archiveArtifacts(
                artifacts: "build-plan.json,resolved-images.env,ingress-traefik.yml,local-modules.tsv,module-sources.tsv,ci-plan/**",
                allowEmptyArchive: true
            )
        }

        failure {
            sh '''
                if [ -n "${RELEASE_DIR:-}" ] && [ -f "$RELEASE_DIR/compose.yml" ]; then
                  docker compose \
                    --env-file "$RELEASE_DIR/release.env" \
                    --file "$RELEASE_DIR/compose.yml" \
                    logs --tail=100 --no-color || true
                fi
            '''
        }
    }
}