# S3-compatible Storage Resource Provider (v0)

Manafield's first object-storage Resource Provider speaks S3 rather than
Cloudflare-specific APIs. Cloudflare R2 is the initial target; other S3
compatible stores can use the same provider when they support SigV4 and
ListObjectsV2. The Provider performs only a read-only bucket readiness check
and registers `storage.object@1.0.0` in Core. It never uploads, deletes, or
proxies Module files.

## Nonsecret instance configuration

```yaml
resources:
  - id: manafield-r2
    provider: s3
    config:
      endpoint: https://YOUR_ACCOUNT.r2.cloudflarestorage.com
      bucket: your-bucket
      region: auto
      forcePathStyle: true
      # publicBaseUrl: https://cdn.example.test/media
```

In the consumer's `manafield.module.json`, declare
`requires.capabilities.media: {id: storage.object, version: "^1.0.0"}`.
The Instance Definition then sets `bindings.media: manafield-r2`. Direct
staging rejects missing or mismatched S3 bindings.

## Private credentials

Place an S3 **Access Key ID** and **Secret Access Key** in the private
Instance root, never in Git or the Build Plan:

```text
secrets/s3/manafield-r2/access-key-id
secrets/s3/manafield-r2/secret-access-key
```

Use restrictive host-directory permissions. The direct executor produces
read-only copies for nonroot containers inside a private `runtime` directory.
Changes to a master credential require a controlled runtime-secret rotation;
this provider does not create or rotate API credentials.

Consumer Modules receive:
```text
MANAFIELD_BINDING_MEDIA_TARGET
MANAFIELD_BINDING_MEDIA_ENDPOINT_URL
MANAFIELD_BINDING_MEDIA_CONFIG_BUCKET
MANAFIELD_BINDING_MEDIA_CONFIG_REGION
MANAFIELD_BINDING_MEDIA_CONFIG_PREFIX
MANAFIELD_BINDING_MEDIA_CONFIG_FORCE_PATH_STYLE
MANAFIELD_BINDING_MEDIA_SECRET_ACCESS_KEY_ID_FILE
MANAFIELD_BINDING_MEDIA_SECRET_SECRET_ACCESS_KEY_FILE
```

Optional: `MANAFIELD_BINDING_MEDIA_CONFIG_PUBLIC_BASE_URL`.
These are endpoint metadata and **secret file paths**, not secret values.
The Module uses its own S3 SDK. R2 does not use S3 public-read object ACLs;
public media hosting needs a separately configured custom domain/CDN.

**Security warning:** A per-module object key prefix is *not* access control.
If two Modules receive the same bucket-wide S3 key, either can access the
other's prefix. This v0 is for **trusted first-party consumer Modules only**.
Untrusted modules need per-module credentials or a prefix-enforcing gateway.

## Deployment and testing

The `s3` Docker Compose profile starts a nonroot Provider observer.
Direct CLI releases support up to one PostgreSQL and one S3 Resource.
Only a root HTTPS S3 endpoint is accepted. Bucket provisioning, public
publication, and object transfer are not performed by the Provider.

```sh
python3 -m unittest discover -s providers/s3 -p 'test_*.py' -v
cargo test --locked --lib
```

The offline tests use fake S3 clients; no R2 tokens or real bucket requests
are used. Jenkins deployment support may require separate executor wiring.
