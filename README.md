# asana-cli

`asana-cli` is an agent-friendly command-line client for the [Asana REST API](https://developers.asana.com/docs/quick-start). It combines ergonomic typed read commands with a guarded raw API interface and predictable JSON on stdout.

Typed commands cover daily task workflows: task CRUD, comments, tags, attachments, sections, subtasks, dependencies, followers, and project placement. `asana api` provides guarded access to the remaining REST API while later resource families are added.

## Build

```bash
cargo build --release
./target/release/asana --help
```

## Authenticate

Create a personal access token in the Asana developer console, then store it in the operating-system keyring:

```bash
asana auth login
asana auth status
```

For CI, scripts, and agents, do not place a token in a config file. Set it only for the process instead:

```bash
ASANA_ACCESS_TOKEN=... asana auth status --json
```

`ASANA_ACCESS_TOKEN` takes precedence over the stored keyring token. `asana auth logout` removes only the stored token.

Use named profiles for multiple accounts or organizations:

```bash
asana --profile work auth login
asana profile use work
asana profile list
```

## Pick a workspace

```bash
asana workspace list
asana workspace use 123456789
```

The selected workspace is stored in the platform configuration directory as `asana-cli/config.toml`. Use `--workspace <gid>` to override it for one command.

## Common commands

```bash
# My open tasks in the selected workspace
asana task list

# Tasks in a project, including completed ones
asana task list --project 123456789 --include-completed

# Discover sections and list open tasks in one section
asana section list --project 123456789
asana task list --section 987654321

# Search task names and descriptions
asana task search "quarterly planning" --assignee me --due-before 2026-09-01

# Full task description, all human comments, and all attachment metadata
asana task get 123456789 --comments --attachments

# Page through comments or attachments
asana task comments 123456789 --limit 50
asana attachment list 123456789 --limit 50

# Download a single file or all task attachments
asana attachment download 123456789 -o ./brief.pdf
asana attachment download-all 123456789 --directory ./downloads

# Create and update work
asana task create --name "Investigate timeout" --project 123 --section 456 --tag 789
asana task update 111 --expect-modified-at 2026-07-11T12:00:00.000Z --complete
asana task comment add 111 --text "Fixed and ready for review"

# Preview and confirm destructive changes
asana --json --dry-run task delete 111
asana --yes task delete 111
```

## JSON and pagination

Pass `--json` to any command for structured output. Individual resources are JSON objects. Collections are always:

```json
{
  "data": [],
  "next_offset": "opaque-asana-offset-or-null"
}
```

Use `next_offset` as `--offset` in the next request. A collection page has a default size of 50 and accepts `--limit 1..100`.

The Asana task-search endpoint does not support stable cursor pagination, so `asana task search` accepts only `--limit` (up to 100). Search also requires Premium access in Asana.

JSON data always goes to stdout. Errors and `--verbose` diagnostics go to stderr. See [the agent contract](docs/agent-contract.md) for exact output and exit-code behavior.

## AI skill

The repository includes an agent skill in [`skills/asana-cli`](skills/asana-cli). Install or link that directory in your agent's skill location to teach it how to query Asana safely through this CLI, handle pagination and errors, and retrieve attachments without exposing credentials.

The proposed roadmap from the current MVP to full public REST API coverage is documented in [`docs/full-api-parity-plan.md`](docs/full-api-parity-plan.md).

## Raw REST API

Use typed commands when they exist. The raw interface provides guarded access to the remaining REST API:

```bash
asana --json api get /tasks/123 --query opt_fields=gid,name,notes
asana --json --dry-run api post /tasks --data @task.json
asana --json api put /tasks/123 --data '{"completed":true}'
asana --json --yes api delete /tasks/123
asana --json api upload /attachments --file ./brief.pdf --field parent=123
```

JSON supplied with `--data` is wrapped in Asana's `data` envelope. Writes are never retried after an ambiguous transport failure. DELETE requires an interactive confirmation or `--yes`; `--dry-run` prints the normalized mutation without authentication or network access.

Known REST resource families are vendored in [`api-support.toml`](api-support.toml). Newly released paths require `--allow-unknown-endpoint` until the catalog is updated.

## Download behavior

Attachment URLs are refreshed immediately before each download. Downloads stream to a temporary file and are renamed only after success. Names derived from Asana are made safe for the chosen directory, existing files are never overwritten, and `download-all` continues after individual failures. It exits with code `5` when any attachment fails.

## Environment variables

| Variable | Purpose |
| --- | --- |
| `ASANA_ACCESS_TOKEN` | Temporary bearer token; overrides keyring. |
| `ASANA_API_BASE_URL` | Override API base URL for local development or a compatible proxy. |

## Scope

The CLI currently uses Asana personal access tokens and process-level token overrides. Typed daily-work mutations and custom-field task search are available. OAuth, typed strategic/admin resources, parameter-level manifest coverage, and caching remain future work.
