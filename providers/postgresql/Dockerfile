FROM postgres:18-alpine

RUN apk add --no-cache curl

COPY register.sh /usr/local/bin/manafield-postgresql-provider
RUN chmod +x /usr/local/bin/manafield-postgresql-provider

ENTRYPOINT ["/usr/local/bin/manafield-postgresql-provider"]