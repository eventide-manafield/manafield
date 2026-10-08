# Manafield PostgreSQL Resource Provider — v0

This directory contains the first concrete Resource Provider implementation.

Its job is deliberately small:

1. observe a PostgreSQL instance that it is configured to own/watch
2. wait until PostgreSQL is reachable
3. register a generic Resource Instance with Manafield Core
4. keep checking both PostgreSQL and the Core registration
5. re-register after a Core restart when the in-memory Registry is empty

It does **not** teach Manafield Core how PostgreSQL works.

## Boundary

Manafield Core only receives generic Resource metadata:

```json
{
  "id": "manafield-postgres",
  "name": "Manafield PostgreSQL",
  "type": "postgresql",
  "provides": {
    "capabilities": [
      {
        "id": "database.postgresql",
        "version": "1.0.0"
      }
    ]
  }
}
```

The following remain outside Core:

- PostgreSQL image/version selection
- port and container details
- storage/volume policy
- database/user creation
- passwords and other secrets
- PostgreSQL health probing
- future allocation of per-Module database/schema/account

The Provider is the component that knows those implementation details and tells Core only which concrete Resource Instance exists and which Capability contract it provides.

## Environment

```text
MANAFIELD_CORE_URL
  default: http://core:8080

MANAFIELD_RESOURCE_ID
  default: manafield-postgres

MANAFIELD_RESOURCE_NAME
  default: Manafield PostgreSQL

POSTGRES_HOST
  default: manafield-postgres

POSTGRES_PORT
  default: 5432

MANAFIELD_POSTGRES_CAPABILITY_VERSION
  default: 1.0.0

MANAFIELD_RESOURCE_CHECK_INTERVAL
  default: 5
```

## Bootstrap integration

On the first Jenkins run, selecting **Manafield PostgreSQL Resource** writes this desired Resource request into the generated Instance Definition:

```yaml
resources:
  - id: manafield-postgres
    enabled: true
    provider: postgresql
```

The Build Plan emits that request to `ci-plan/resources.tsv`. Jenkins builds this Provider image, activates the `postgresql` Compose profile, and waits until Core reports the registered Resource through `GET /resources`.

The desired request and the live registration are intentionally separate: the Instance Definition asks for the Resource, while this Provider proves that the concrete Resource is actually available.

## Manual example

The bundled `compose.example.yml` starts a disposable/example PostgreSQL instance and this Provider on the existing `manafield-modules` network.

It expects Manafield Core to already be running on that network as `core:8080`.

```bash
docker compose -f providers/postgresql/compose.example.yml up -d --build
```

Then:

```bash
curl http://127.0.0.1:18080/resources
```

should eventually contain `manafield-postgres`.

## Status

This is a bootstrap/reference implementation, not the final Resource Provider lifecycle protocol.

The current v0 deployment path can additionally consume a generated allocation manifest for explicit Module bindings. For each PostgreSQL-bound Requirement slot, the deployment adapter can persist a private password file, request a Module-scoped schema/role inside the shared Resource database, and inject slot-scoped endpoint/config/secret-file metadata into the consumer Module.

Resource removal policy, allocation cleanup, credential rotation, richer Resource health state, and a provider-neutral secret materialization protocol remain future work.
