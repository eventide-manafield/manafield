import groovy.json.JsonOutput
import groovy.json.JsonSlurperClassic

def buildPlan = null
def moduleBuilds = [:]

def safeComponentId(String value) {
    return value.toLowerCase().replaceAll(/[^a-z0-9_.-]+/, "-")
}

pipeline {
    agent any

    options {
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

                sh """
                    set -eu

                    test -f "${env.INSTANCE_ROOT}/instance.yaml"

                    docker build \
                      --target build-plan-runtime \
                      --tag "manafield-build-plan:${env.CORE_SHA}" \
                      .

                    resolver_container="\$(docker create "manafield-build-plan:${env.CORE_SHA}")"
                    trap 'docker rm -f "\$resolver_container" >/dev/null 2>&1 || true' EXIT

                    docker cp \
                      "\$resolver_container:/usr/local/bin/manafield-build-plan" \
                      ./manafield-build-plan

                    chmod +x ./manafield-build-plan
                    ./manafield-build-plan \
                      "${env.INSTANCE_ROOT}/instance.yaml" \
                      build-plan.json
                """

                script {
                    buildPlan = new JsonSlurperClassic().parseText(readFile("build-plan.json"))

                    if (buildPlan.version != 0) {
                        error("Unsupported Build Plan version: ${buildPlan.version}")
                    }

                    if (!buildPlan.modules.any { it.id == "reference-web" }) {
                        error("Bootstrap deployment currently requires the reference-web Module")
                    }
                }
            }
        }

        stage("Verify Core") {
            steps {
                sh """
                    set -eu

                    docker build \
                      --target verify \
                      --tag "manafield-core-verify:${env.CORE_SHA}" \
                      .
                """
            }
        }

        stage("Checkout Modules") {
            steps {
                script {
                    buildPlan.modules.each { module ->
                        def safeId = safeComponentId(module.id as String)
                        def relativeDir = "modules/${safeId}"
                        def absoluteDir = "${pwd()}/${relativeDir}"

                        dir(relativeDir) {
                            deleteDir()
                        }

                        withEnv([
                            "MODULE_SOURCE_URL=${module.source.repository}",
                            "MODULE_SOURCE_REF=${module.source.ref}",
                            "MODULE_SOURCE_DIR=${absoluteDir}"
                        ]) {
                            sh '''
                                set -eu

                                git clone --no-checkout "$MODULE_SOURCE_URL" "$MODULE_SOURCE_DIR"
                                git -C "$MODULE_SOURCE_DIR" fetch --depth=1 origin "$MODULE_SOURCE_REF"
                                git -C "$MODULE_SOURCE_DIR" checkout --detach FETCH_HEAD
                            '''
                        }

                        def revision = sh(
                            script: "git -C '${absoluteDir}' rev-parse --short=12 HEAD",
                            returnStdout: true
                        ).trim()

                        moduleBuilds[module.id] = [
                            id: module.id,
                            safeId: safeId,
                            dir: absoluteDir,
                            revision: revision,
                            image: "manafield-module-${safeId}:${revision}"
                        ]
                    }
                }
            }
        }

        stage("Build Images") {
            steps {
                sh """
                    set -eu

                    docker build \
                      --target core-runtime \
                      --label "org.opencontainers.image.revision=${env.CORE_SHA}" \
                      --tag "${env.CORE_IMAGE}" \
                      .
                """

                script {
                    buildPlan.modules.each { module ->
                        def built = moduleBuilds[module.id]

                        if (module.build.type != "docker") {
                            error("Unsupported Module build type: ${module.build.type}")
                        }

                        def contextPath = "${built.dir}/${module.build.context}"
                        def dockerfilePath = "${built.dir}/${module.build.dockerfile}"

                        withEnv([
                            "MODULE_IMAGE=${built.image}",
                            "MODULE_CONTEXT=${contextPath}",
                            "MODULE_DOCKERFILE=${dockerfilePath}",
                            "MODULE_REVISION=${built.revision}"
                        ]) {
                            sh '''
                                set -eu

                                docker build \
                                  --label "org.opencontainers.image.revision=$MODULE_REVISION" \
                                  --tag "$MODULE_IMAGE" \
                                  --file "$MODULE_DOCKERFILE" \
                                  "$MODULE_CONTEXT"
                            '''
                        }
                    }

                    writeFile(
                        file: "resolved-images.json",
                        text: JsonOutput.prettyPrint(JsonOutput.toJson([
                            core: [
                                image: env.CORE_IMAGE,
                                revision: env.CORE_SHA
                            ],
                            modules: moduleBuilds
                        ])) + "\n"
                    )
                }
            }
        }

        stage("Stage Release") {
            steps {
                script {
                    env.RELEASE_DIR = "${env.INSTANCE_ROOT}/releases/${env.BUILD_NUMBER}"
                    def referenceImage = moduleBuilds["reference-web"].image
                    def network = buildPlan.deployment.network

                    withEnv([
                        "REFERENCE_IMAGE=${referenceImage}",
                        "MANAFIELD_NETWORK_VALUE=${network}"
                    ]) {
                        sh '''
                            set -eu

                            mkdir -p "$RELEASE_DIR/modules"

                            cp deploy/compose.yml "$RELEASE_DIR/compose.yml"
                            cp build-plan.json "$RELEASE_DIR/build-plan.json"
                            cp resolved-images.json "$RELEASE_DIR/resolved-images.json"

                            printf '%s\n' \
                              "MANAFIELD_CORE_IMAGE=$CORE_IMAGE" \
                              "MANAFIELD_REFERENCE_IMAGE=$REFERENCE_IMAGE" \
                              "MANAFIELD_MODULES_PATH=$RELEASE_DIR/modules" \
                              "MANAFIELD_NETWORK=$MANAFIELD_NETWORK_VALUE" \
                              > "$RELEASE_DIR/release.env"
                        '''
                    }

                    buildPlan.modules.each { module ->
                        def built = moduleBuilds[module.id]
                        def manifest = "${built.dir}/manafield.module.json"
                        def destination = "${env.RELEASE_DIR}/modules/${built.safeId}"

                        withEnv([
                            "MODULE_MANIFEST=${manifest}",
                            "MODULE_DESTINATION=${destination}"
                        ]) {
                            sh '''
                                set -eu

                                test -f "$MODULE_MANIFEST"
                                mkdir -p "$MODULE_DESTINATION"
                                cp "$MODULE_MANIFEST" "$MODULE_DESTINATION/manafield.module.json"
                            '''
                        }
                    }
                }
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
                        && grep -Fq '"id":"reference-web"' /tmp/manafield-modules.json \
                        && docker exec "$core_container" \
                           curl --fail --silent --show-error \
                           http://reference-web:8080/manafield/health >/tmp/manafield-reference-health.json \
                        && docker exec "$core_container" \
                           curl --fail --silent --show-error \
                           http://reference-web:8080/api/core/modules >/tmp/manafield-reference-modules.json \
                        && grep -Fq '"id":"reference-web"' /tmp/manafield-reference-modules.json
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
                artifacts: "build-plan.json,resolved-images.json",
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
