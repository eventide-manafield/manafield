# Account Core email signup deployment (explicit opt-in)

This is an **operator-reviewed release procedure**, not an instruction to deploy by merging a PR. Keep the existing `admin` account and Manage OAuth available.

## Dependencies
1. Review and merge `manafield-account` PR #16 (immutable handles, one-time OAuth PKCE) and then PR #17 (email-verified registration, stacked on #16).
2. Sync the reviewed Account Core **source commit** into the private `LOCAL_MODULES_ROOT/manafield-account-core` directory. The current private Instance declares `source.type: dir`; merging a public GitHub PR alone **does not update the deployed directory Module**.
3. Retain a backup of the private Instance definition, Account PostgreSQL data, and the previous Account Core container digest. Review schema migrations before replacing an image.
4. The account module's `scratch` image must include a CA certificate bundle to validate `smtp.resend.com:465` over TLS.

## SMTP secret
The operator provisioned a Resend **sending** API key at the following private, on-host path:

```text
$INSTANCE_ROOT/secrets/account-smtp/password
```

The pipeline never reads or prints its contents; in enabled mode it requires an existing nonempty, non-symlinked file and bind-mounts it as **read-only** to:

```text
/run/manafield/signup/smtp-password
```

The Account Core process runs under UID 65532. The operator must verify its **read access** to the bind-mounted key (ACL or narrowly scoped permissions) without granting blanket readability and without printing the key. Do not put the key in the Dockerfile, Jenkins parameters, GitHub repo, `instance.yaml`, or logs.

## Default-off behavior
The new Jenkins boolean parameter `ENABLE_ACCOUNT_SIGNUP` defaults to **false**.
- **False:** deploy `MANAFIELD_ACCOUNT_REGISTRATION_ENABLED=false`; do not configure SMTP variables or mount the SMTP key. The register/verify HTTP handlers respond 503; the login page does not advertise registration.
- **True:** require the above on-host file; render SMTP settings (host `smtp.resend.com`, port 465, required implicit TLS, sender `system@manafield.studio`) and set the registration flag to `true`. Only Account Core receives the key. Noctemblith and other Modules do not.

Changing the parameter affects **only a newly staged/deployed release**, not running containers. On subsequent Jenkins builds the default is false again unless explicitly selected.

## Staging / production gates
1. Confirm disposable PostgreSQL 18 CI migration + HTTP tests for the reviewed PR #17; test creation, verification, login, token reuse and expiration, verified-email conflicts, reserved-handle rejection, legacy accounts, and disabled registration.
2. Run the same module version in a disposable isolated Instance/DB, **not the active `admin` database**. Perform a real confirmation email with a throwaway mailbox and verify successful HTTPS link + POST + login.
3. Render Compose and verify that the disabled configuration contains neither SMTP secret mount nor activation; after operator opt-in, verify only the Account Core service includes the secret mount and values (never output the secret).
4. Approve production backup + rollout separately. Confirm existing `admin` login, Manage PKCE SSO, and current sessions afterward.
5. Manually test `eventide` signup with email verification. The previously created Noctemblith `eventide` **must not** be linked by username alone; authenticated two-sided linking is a separate release.

## Rollback
Re-deploy with `ENABLE_ACCOUNT_SIGNUP=false`, verify register/verify are 503 and existing login remains usable. Review any in-flight pending registrations, which retain their 30-minute expiry, and apply a separate account rollback plan for schema incompatibilities. Do not delete or rename existing identities.
