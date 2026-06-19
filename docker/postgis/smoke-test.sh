#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 IMAGE" >&2
    exit 2
fi

image="$1"
container="postile-smoke-${RANDOM}-${RANDOM}"

cleanup() {
    docker rm -f "$container" >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker run \
    --detach \
    --name "$container" \
    --env POSTGRES_PASSWORD=postgres \
    "$image" >/dev/null

for _ in $(seq 1 60); do
    if docker exec "$container" pg_isready -U postgres -d postgres >/dev/null 2>&1; then
        break
    fi
    sleep 1
done

docker exec "$container" pg_isready -U postgres -d postgres

docker exec "$container" psql -U postgres -d postgres -v ON_ERROR_STOP=1 <<'SQL'
SELECT postgis_lib_version();
SELECT pt_version();
DO $$
BEGIN
    IF (SELECT pt_version()) <> (SELECT extversion FROM pg_extension WHERE extname = 'postile') THEN
        RAISE EXCEPTION 'pt_version() does not match installed extension version';
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'postgis') THEN
        RAISE EXCEPTION 'postgis extension is not installed';
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'postile') THEN
        RAISE EXCEPTION 'postile extension is not installed';
    END IF;
END
$$;
SQL
