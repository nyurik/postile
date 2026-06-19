# syntax=docker/dockerfile:1.7

ARG BASE_IMAGE
ARG PG_MAJOR
ARG POSTILE_VERSION
ARG VCS_REF

FROM ${BASE_IMAGE}

ARG BASE_IMAGE
ARG PG_MAJOR
ARG POSTILE_VERSION
ARG VCS_REF

LABEL org.opencontainers.image.title="PostGIS with Postile" \
      org.opencontainers.image.description="PostGIS image with the Postile PostgreSQL extension installed" \
      org.opencontainers.image.source="https://github.com/nyurik/postile" \
      org.opencontainers.image.revision="${VCS_REF}" \
      org.opencontainers.image.version="${POSTILE_VERSION}" \
      org.opencontainers.image.base.name="${BASE_IMAGE}"

USER root

COPY target/release/postile-pg${PG_MAJOR}.tar.gz /tmp/postile.tar.gz
COPY --chmod=755 docker/postgis/initdb-postile.sh /docker-entrypoint-initdb.d/20_postile.sh

RUN set -eux; \
    mkdir -p /tmp/postile; \
    tar -xzf /tmp/postile.tar.gz -C /tmp/postile; \
    rm /tmp/postile.tar.gz; \
    install -m 755 -D "$(find /tmp/postile -type f -name 'postile.so' -print -quit)" "$(pg_config --pkglibdir)/postile.so"; \
    install -m 644 -D "$(find /tmp/postile -type f -name 'postile.control' -print -quit)" "$(pg_config --sharedir)/extension/postile.control"; \
    find /tmp/postile -type f -name 'postile--*.sql' -exec install -m 644 -t "$(pg_config --sharedir)/extension" {} \;; \
    rm -rf /tmp/postile; \
    test -f "$(pg_config --pkglibdir)/postile.so"; \
    test -f "$(pg_config --sharedir)/extension/postile.control"; \
    ls "$(pg_config --sharedir)"/extension/postile--*.sql >/dev/null

USER postgres
