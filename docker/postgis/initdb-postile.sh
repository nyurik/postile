#!/usr/bin/env bash
set -euo pipefail

# Perform all actions as $POSTGRES_USER, matching the upstream PostGIS init script.
export PGUSER="$POSTGRES_USER"

for DB in template_postgis "$POSTGRES_DB"; do
    echo "Loading Postile extension into $DB"
    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --no-password --no-psqlrc --dbname="$DB" --command='CREATE EXTENSION IF NOT EXISTS postile;'
done
