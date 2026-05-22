#!/usr/bin/env bash
set -euo pipefail

postgres_user="${POSTGRES_USER:-tusker}"
postgres_password="${POSTGRES_PASSWORD:-tusker123}"
postgres_db="${POSTGRES_DB:-tusker}"
postgres_port="${POSTGRES_PORT:-5432}"

for postgres_version in 16 17 18; do
    postgres_host="postgres${postgres_version}"
    pg_url="host=${postgres_host} port=${postgres_port} user=${postgres_user} password=${postgres_password} dbname=${postgres_db}"

    echo "Running cargo test against PostgreSQL ${postgres_version} (${postgres_host})"
    PGHOST="${postgres_host}" \
        PGPORT="${postgres_port}" \
        PGUSER="${postgres_user}" \
        PGPASSWORD="${postgres_password}" \
        PGDATABASE="${postgres_db}" \
        PG_URL="${pg_url}" \
        cargo test --all-features --workspace "$@"
done
