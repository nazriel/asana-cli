# Phase 2 verification

Local evidence for `asana` 0.4:

- unit and integration tests cover parser, normalized mutation previews, real HTTP methods/paths, request envelopes, and no-body access decisions;
- every Phase 2 mutation is included in the normalized endpoint matrix in `tests/cli_requests.rs`;
- Phase 2 read families are exercised against a local HTTP server in `tests/cli_requests.rs`;
- `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo build --release` pass;
- `skills/asana-cli` passes the skill validator;
- command help, agent guidance, migration notes, and `api-support.toml` are updated.

## Live sandbox gate

The opt-in suite is the `smoke:phase2` mise task ([`.mise/tasks/smoke/phase2`](../.mise/tasks/smoke/phase2)). It refuses to run unless all required sandbox variables and the exact confirmation value are present. It creates uniquely named projects, briefs, status updates, templates, duplicates, instantiated resources, and optional memberships/tasks. Its exit trap deletes only GIDs captured from resources created by that run.

Configure authentication locally; never put a token in arguments or commit it:

```bash
asana --profile sandbox auth login
ASANA_SMOKE_PROFILE=sandbox \
ASANA_SMOKE_WORKSPACE=<sandbox-workspace-gid> \
ASANA_SMOKE_TEAM=<sandbox-team-gid> \
ASANA_SMOKE_CONFIRM=phase2-sandbox \
mise run smoke:phase2
```

Optional variables:

- `ASANA_SMOKE_MEMBER`: a sandbox user GID used to verify generic membership CRUD;
- `ASANA_SMOKE_TASK_TEMPLATE`: an existing sandbox task-template GID used to verify get/list/instantiate and cleanup of the instantiated task;
- `ASANA_BIN`: path to the binary, defaulting to `asana`.

Do not point this suite at a normal workspace. Access-request decisions, workspace/team user removal, workspace rename, and user profile updates are intentionally excluded from live automation because they cannot be proven self-contained. Their request shapes and safety gates are covered locally.
