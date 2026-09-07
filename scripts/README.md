# Development scripts

## Phase 0 WSL gate

The release gate runs without containers and refuses repositories located under
`/mnt/*`. Clone the repository into the WSL native filesystem first.

Required native tools:

- Rust 1.94.0 selected by `rust-toolchain.toml`;
- PostgreSQL server/client;
- Redis server/client;
- Python 3.

Prepare the dedicated PostgreSQL smoke-test database once:

```bash
./scripts/prepare-wsl-test-db.sh
export NLI_TEST_DATABASE_URL='postgresql://<wsl-user>@localhost/nli_v2_p0_test?host=/var/run/postgresql'
```

Then run:

```bash
./scripts/wsl-gate.sh
```

The gate starts its own loopback-only, non-persistent Redis process and removes
it on exit. It does not read `.env`. The PostgreSQL URL is required explicitly,
and the Rust smoke test refuses database names that do not end in `_test`.

`scripts/wsl-gate.sh` is the authoritative P0-03 gate. A plain
`cargo test --all-targets` intentionally excludes the service smoke target;
only the gate enables `dependency-tests`, verifies that at least two tests ran,
and rejects any ignored dependency test.

P0-03 covers Cargo/toolchain and native dependency smoke tests only. OpenAPI,
route-manifest, release-manifest, and deployment checks are added by the later
Phase 0 slices recorded in `doc/v2/progress/phase-0.md`.
