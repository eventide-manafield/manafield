# Static Web Resource Provider — Go v0 (isolated prototype)

Status: working development proof of concept, NOT installed/deployed into the live Manafield Instance. This folder is committed on a feature branch only; it has not been merged or deployed to a live Instance.

## Confirmed intent

- A dedicated Static Web Resource rather than a generic file-storage Resource.
- Provider implementation is Go.
- Provider config contains an explicit allowlist of absolute directories; it will not create missing directories.
- A Resource is the concrete directory; a Provider manages it. An HTTP handler serves files; Core does not proxy file bytes.
- Only an Instance's explicit Web exposure binding publishes a Resource. The alias replaces the default ID-based path; no automatic second path.
- Public paths follow /resource/{alias}/{file}. Default alias = Resource ID.
- Changed or newly added files are read on the next request, without image rebuilding or server restarting.
- File not found / hidden / directory / unexposed alias -> 404.
- Configured Resource root missing or unavailable -> 503 for that Resource only. Others continue to work.
- No separate user-permission module. v0 has public-only exposures; anyone who can reach the published URI may GET the file.

## Current example

Example files and configurations are in examples/. The two JSON files are distinct by design:

| File | Owner | Meaning |
| --- | --- | --- |
| examples/provider.json | Provider administrator | Resource ID → pre-existing absolute directory |
| examples/instance-web.json | Instance deployment policy | Resource ID → public Web alias |

This separate instance-web.json is a temporary v0 adapter/stand-in for the Instance binding interface, not the existing Manafield Instance YAML schema. Proper integration into the release plan, Module requirements, and Web Exposure routing remains pending.

Example provider.json deliberately leaves core_url empty; this runs without contacting live Core. It contains a missing-assets directory on purpose, to demonstrate isolated 503.

Run from this directory:

    GO=go  # Go 1.27+ required
    GOTOOLCHAIN=local GOPROXY=off "$GO" test ./...
    GOTOOLCHAIN=local GOPROXY=off "$GO" run . -provider-config examples/provider.json -web-bindings examples/instance-web.json

On the server in another shell:

    curl -i http://127.0.0.1:18580/resource/icons/logo.svg
    curl -i http://127.0.0.1:18580/resource/home/index.html
    curl -i http://127.0.0.1:18580/resource/home/site.css
    curl -i http://127.0.0.1:18580/resource/home/not-found.css
    curl -i http://127.0.0.1:18580/resource/unavailable/a.css
    curl -s http://127.0.0.1:18580/healthz

Before running this repository version, create sample asset directories and copy fixtures to the absolute paths configured in examples/provider.json (or adjust the configuration to point to existing directories).

Only 127.0.0.1:18580 is listened to by the example. It is NOT accessible via the public Manafield domain.

## Optional Core v0 registration

If the trusted operator explicitly sets core_url (for example, http://core:8080 in a private network), the binary reconciles available Resource IDs every 15 seconds against the generic GET/POST /resources API. It registers them as Resource Instances with a provisional capability contract, web.static-files@1.0.0.

This does NOT implement Core Module binding validation/injection, automatic Module shutdown, Resource removal, or Core health-state transitions. Core may retain metadata after a directory later disappears; the HTTP route itself will respond 503. A 409 conflict is reported as such, not silently accepted. These lifecycle gaps must be closed before deploying as a production Resource Provider.

## Filesystem and HTTP guardrails

- Resource directories must already exist, be absolute, and be non-symlink directories (unavailable roots are isolated, not fatal).
- Files are accessed using Go's os.Root path-confining APIs; path escape, including nested symlinks to outside the root, is blocked.
- Requests containing hidden segments or traversal attempts are denied. No directory listings, no implicit index resolution, no special/device files.
- Only GET and HEAD are allowed. Cache-Control: no-store and X-Content-Type-Options: nosniff are applied.
- No directory or file enumeration endpoints. /healthz provides counts, not filesystem paths.
- No write/modify HTTP API. The filesystem owner deploys or edits static files separately.
- Authentication is NOT supplied for public aliases; do not register private files.
- Bind-mounted containers should have only approved directories mapped read-only and run as non-root. Admin-controlled provider configuration must not be writable by untrusted modules.

## Deliberately out of scope

- Static release history/rollback and file upload API
- General-purpose File System Resource (separate future Provider)
- Automatic Web publication from a mere Capability binding
- Host/domain routing changes to the real Instance or Traefik configuration
- Replacing Manafield Home's current Go embed.FS implementation
- Per-user authenticated/private file serving
- Official Docker/CI rollout (builds should still work without Jenkins)

## Next integration step

Connect Instance-owned public aliases and the actual Module requires.capabilities slot to this v0 runtime without changing the accepted Web Surface/Capability contracts. Build a staged release and check host/prefix collisions before changing live routing. Once bound, test a real Home CSS replacement and verify that neither image rebuild nor container restart occurs.
