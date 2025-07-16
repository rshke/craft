#!/usr/bin/env bash
set -x
set -eo pipefail

if ! [ -x "$(command -v sqlx)" ]; then
    echo >&2 "Error: sqlx is not installed."
    exit 1
fi

if ! [ -x "$(command -v psql)" ]; then
    echo >&2 "Error: psql is not installed."
    exit 1
fi

if ! [ -x "$(command -v docker)" ]; then
    echo >&2 "Error: docker is not installed."
    exit 1
fi

DB_USER=${POSTGRES_USER:=postgres}
DB_PASSWORD="${POSTGRES_PASSWORD:=password}"
DB_NAME="${POSTGRES_DB:=craft}"
DB_PORT="${POSTGRES_PORT:=5432}"

CONTAINER_ID=$(docker ps --filter "ancestor=postgres" --filter "status=running" -q)

if [ -z "$CONTAINER_ID" ]; then
    echo "Postgres container not running. Starting one..."
    docker run \
        -e POSTGRES_USER=${DB_USER} \
        -e POSTGRES_PASSWORD=${DB_PASSWORD} \
        -p "${DB_PORT}":5432 \
        -d postgres \
        postgres -N 1000
    # sleep 3
    until psql -h "localhost" -U "${DB_USER}" -p "${DB_PORT}" -d "postgres" -c '\q'; do
        >&2 echo "Postgres is still unavailable - sleeping"
        sleep 1
    done

    CONTAINER_ID=$(docker ps --filter "ancestor=postgres" --filter "status=running" -q)
fi

echo "Postgres container is running. Checking database..."

DB_EXISTS=$(docker exec -u postgres "$CONTAINER_ID" psql -tAc "SELECT 1 FROM pg_database WHERE datname='${DB_NAME}'")
if [ "$DB_EXISTS" = "1" ]; then
    echo "Database '${DB_NAME}' already exists. Skipping creation."
else
    echo "Database '${DB_NAME}' does not exist. Creating..."
    docker exec -u postgres "$CONTAINER_ID" psql -c "CREATE DATABASE \"${DB_NAME}\";"
    echo "Database '${DB_NAME}' created."
fi

export DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD}@localhost:${DB_PORT}/${DB_NAME}
# sqlx database create
sqlx migrate run
echo "Postgres has been migrated, ready to go!"
