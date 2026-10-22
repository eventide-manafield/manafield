FROM alpine:3.22

RUN apk add --no-cache curl postgresql-client \
    && mkdir -p /run/manafield/postgresql/secrets

COPY register.sh /usr/local/bin/manafield-postgresql-provider
RUN chmod 0755 /usr/local/bin/manafield-postgresql-provider

ENTRYPOINT ["/usr/local/bin/manafield-postgresql-provider"]
