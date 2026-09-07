#!/usr/bin/env bash
set -Eeuo pipefail

fail() {
  printf 'WSL_DB_SETUP_ERROR: %s\n' "$1" >&2
  exit 1
}

if ! grep -qi microsoft /proc/sys/kernel/osrelease; then
  fail 'this helper must run inside WSL2'
fi
command -v psql >/dev/null || fail 'install the native PostgreSQL server/client packages first'
command -v createdb >/dev/null || fail 'createdb is required'
command -v pg_isready >/dev/null || fail 'pg_isready is required'

service_name="${NLI_TEST_POSTGRES_SERVICE:-postgresql}"
[[ "$service_name" =~ ^[a-z0-9][a-z0-9_-]*$ ]] || fail 'NLI_TEST_POSTGRES_SERVICE contains unsafe characters'
sudo service "$service_name" start >/dev/null
pg_isready -q || fail 'PostgreSQL did not become ready'

role_name="$(id -un)"
database_name="${NLI_TEST_DATABASE_NAME:-nli_v2_p0_test}"
[[ "$role_name" =~ ^[a-z_][a-z0-9_]*$ ]] || fail 'the WSL user name is not safe for automated PostgreSQL setup'
[[ "$database_name" =~ ^[a-z_][a-z0-9_]*_test$ ]] || fail 'NLI_TEST_DATABASE_NAME must be a lowercase identifier ending in _test'

if ! sudo -u postgres psql -tAc "SELECT 1 FROM pg_roles WHERE rolname = '${role_name}'" | grep -qx 1; then
  sudo -u postgres createuser --no-createdb --no-createrole --no-superuser "$role_name"
fi
if ! sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname = '${database_name}'" | grep -qx 1; then
  sudo -u postgres createdb --owner "$role_name" "$database_name"
fi

database_owner="$(sudo -u postgres psql -tAc "SELECT pg_catalog.pg_get_userbyid(datdba) FROM pg_database WHERE datname = '${database_name}'")"
[[ "$database_owner" == "$role_name" ]] || fail 'the existing test database is not owned by the current WSL user'

printf "PostgreSQL test database is ready. Run:\n"
printf "export NLI_TEST_DATABASE_URL='postgresql://%s@localhost/%s?host=/var/run/postgresql'\n" "$role_name" "$database_name"
