FROM alpine:3.22@sha256:822614ea121e1d40a33655081af10ef85f0aa2ab9e61f649ceaf0c651822433c
RUN apk upgrade --no-cache && apk add --no-cache curl postgresql-client && mkdir -p /run/manafield/postgresql/secrets
COPY --chmod=0755 register.sh /usr/local/bin/manafield-postgresql-provider
HEALTHCHECK --interval=2s --timeout=2s --start-period=5s --retries=60 CMD test -f /tmp/manafield-postgresql-ready
ENTRYPOINT ["/usr/local/bin/manafield-postgresql-provider"]
