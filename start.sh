#!/usr/bin/env bash
# Starts budget-helper-back-r for local development: ensures .env exists,
# ensures the target database exists on an already-running MariaDB/MySQL
# server, then runs the app (which applies migrations itself on startup).
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

if [ ! -f .env ]; then
    cp .env.example .env
    echo "note: created .env from .env.example — review JWT_SECRET before using this beyond local dev"
fi

set -a
source .env
set +a

if [[ ! "$DATABASE_URL" =~ ^mysql://([^:]+):([^@]+)@([^:]+):([0-9]+)/(.+)$ ]]; then
    echo "error: DATABASE_URL is not in the expected mysql://user:pass@host:port/dbname format" >&2
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
