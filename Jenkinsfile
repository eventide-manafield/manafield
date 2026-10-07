pipeline {
    agent any

    options {
        skipDefaultCheckout(true)
        disableConcurrentBuilds()
        timestamps()
        buildDiscarder(logRotator(numToKeepStr: "20"))
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

        stage("Resolve Build Plan") {
            steps {
                script {
                    if (!params.INSTANCE_ROOT?.trim()) {
                        error("Jenkins Job must define a non-empty String parameter named INSTANCE_ROOT")
                    }

                    env.INSTANCE_ROOT = params.INSTANCE_ROOT.trim()
                }

                sh '''
                    set -eu

                    test -f "$INSTANCE_ROOT/instance.yaml"
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

                    while IFS="$tab" read -r module_id repository source_ref build_type build_context dockerfile; do
                      [ -n "$module_id" ] || continue

                      safe_id="$(
                        printf '%s' "$module_id" \
                          | tr '[:upper:]' '[:lower:]' \
                          | sed 's/[^a-z0-9_.-]/-/g'
                      )"

                      module_dir="$WORKSPACE/modules/$safe_id"

                      git clone --no-checkout "$repository" "$module_dir"
                      git -C "$module_dir" fetch --depth=1 origin "$source_ref"
                      git -C "$module_dir" checkout --detach FETCH_HEAD

                      revision="$(git -C "$module_dir" rev-parse --short=12 HEAD)"
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
                artifacts: "build-plan.json,resolved-images.env,ci-plan/**",
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
