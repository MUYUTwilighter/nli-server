#!/usr/bin/env bash
set -Eeuo pipefail

fail() {
  printf 'WSL_GATE_ERROR: %s\n' "$1" >&2
  exit 1
}

if ! grep -qi microsoft /proc/sys/kernel/osrelease; then
  fail 'this gate must run inside WSL2'
fi

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
case "$repo_root" in
  /mnt/*) fail 'clone the repository into the WSL native Linux filesystem before running this gate' ;;
esac
cd "$repo_root"

[[ -n "${NLI_TEST_DATABASE_URL:-}" ]] || fail 'NLI_TEST_DATABASE_URL is required and must name a dedicated database ending in _test'
command -v cargo >/dev/null || fail 'cargo is required'
command -v rustc >/dev/null || fail 'rustc is required'
command -v redis-server >/dev/null || fail 'redis-server is required'
command -v redis-cli >/dev/null || fail 'redis-cli is required'
command -v python3 >/dev/null || fail 'python3 is required'

rust_version="$(rustc --version)"
[[ "$rust_version" == rustc\ 1.94.0\ * ]] || fail "rust-toolchain.toml must select rustc 1.94.0 (got: $rust_version)"

work_dir="$(mktemp -d)"
redis_pid=''
cleanup() {
  if [[ -n "$redis_pid" ]] && kill -0 "$redis_pid" 2>/dev/null; then
    kill "$redis_pid" 2>/dev/null || true
    wait "$redis_pid" 2>/dev/null || true
  fi
  rm -rf -- "$work_dir"
}
trap cleanup EXIT INT TERM

redis_port="$(python3 - <<'PY'
import socket
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 0))
    print(sock.getsockname()[1])
PY
)"

redis-server \
  --bind 127.0.0.1 \
  --protected-mode yes \
  --port "$redis_port" \
  --save '' \
  --appendonly no \
  --dir "$work_dir" \
  --dbfilename disabled.rdb \
  --pidfile "$work_dir/redis.pid" \
  >"$work_dir/redis.log" 2>&1 &
redis_pid="$!"

for _ in $(seq 1 50); do
  if redis-cli -h 127.0.0.1 -p "$redis_port" ping >/dev/null 2>&1; then
    break
  fi
  if ! kill -0 "$redis_pid" 2>/dev/null; then
    fail 'ephemeral Redis exited during startup'
  fi
  sleep 0.1
done
redis-cli -h 127.0.0.1 -p "$redis_port" ping >/dev/null 2>&1 || fail 'ephemeral Redis did not become ready'
export NLI_TEST_REDIS_URL="redis://127.0.0.1:${redis_port}/"

cargo fmt --all -- --check
cargo build --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
if ! dependency_output="$(cargo test --locked --features dependency-tests --test dependency_smoke -- --test-threads=1 2>&1)"; then
  printf '%s\n' "$dependency_output" >&2
  fail 'native dependency tests failed'
fi
printf '%s\n' "$dependency_output"
summary="$(grep '^test result:' <<<"$dependency_output" | tail -n 1)"
result_pattern='([0-9]+) passed; ([0-9]+) failed; ([0-9]+) ignored'
[[ "$summary" =~ $result_pattern ]] || fail 'could not parse the dependency test count'
passed="${BASH_REMATCH[1]}"
failed="${BASH_REMATCH[2]}"
ignored="${BASH_REMATCH[3]}"
(( passed >= 2 && failed == 0 && ignored == 0 )) || fail 'dependency tests must execute at least two tests with none failed or ignored'

printf 'WSL_GATE_OK slice=P0-03 rust=1.94.0 dependency_tests=%s\n' "$passed"
