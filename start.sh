#!/usr/bin/env bash
# Starts budget-helper-back-r for local development: ensures the target
# database exists on an already-running MariaDB/MySQL server, then runs the
# app (which applies migrations itself on startup). Config comes from
# config/application.yaml overlaid by config/application-${APP_ENV}.yaml
# (APP_ENV defaults to "dev" here — see src/config.rs).
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo not found on PATH — install Rust: https://rustup.rs" >&2
    exit 1
fi

MYSQL_CLIENT=""
if command -v mysql >/dev/null 2>&1; then
    MYSQL_CLIENT="mysql"
elif command -v mariadb >/dev/null 2>&1; then
    MYSQL_CLIENT="mariadb"
else
    echo "error: no mysql/mariadb client found on PATH — install one to let this script create the database" >&2
    exit 1
fi

export APP_ENV="${APP_ENV:-dev}"

DATABASE_URL=$(grep -h '^database_url:' config/application.yaml "config/application-${APP_ENV}.yaml" 2>/dev/null \
    | tail -n1 \
    | sed -E 's/^database_url:[[:space:]]*"?([^"[:space:]]+)"?.*/\1/')

if [ -z "$DATABASE_URL" ]; then
    echo "error: no database_url found in config/application.yaml or config/application-${APP_ENV}.yaml" >&2
    exit 1
fi

if [[ ! "$DATABASE_URL" =~ ^mysql://([^:]+):([^@]+)@([^:]+):([0-9]+)/(.+)$ ]]; then
    echo "error: database_url is not in the expected mysql://user:pass@host:port/dbname format" >&2
    exit 1
fi
DB_USER="${BASH_REMATCH[1]}"
DB_PASS="${BASH_REMATCH[2]}"
DB_HOST="${BASH_REMATCH[3]}"
DB_PORT="${BASH_REMATCH[4]}"
DB_NAME="${BASH_REMATCH[5]}"

if ! "$MYSQL_CLIENT" -h "$DB_HOST" -P "$DB_PORT" -u "$DB_USER" -p"$DB_PASS" \
    -e "CREATE DATABASE IF NOT EXISTS \`$DB_NAME\`;" 2>/dev/null; then
    echo "error: could not reach MariaDB at $DB_HOST:$DB_PORT — is it running?" >&2
    exit 1
fi

exec cargo run
