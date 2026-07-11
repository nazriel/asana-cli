# asana-cli

`asana-cli` is a small, read-only command-line client for the [Asana REST API](https://developers.asana.com/docs/quick-start). It is designed for direct terminal use and for agents that need predictable JSON on stdout.

This is an MVP/POC: it can read tasks, search them, show task descriptions and comments, inspect attachments, and download attachment files. It does not create or edit Asana data.

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

## Download behavior

Attachment URLs are refreshed immediately before each download. Downloads stream to a temporary file and are renamed only after success. Names derived from Asana are made safe for the chosen directory, existing files are never overwritten, and `download-all` continues after individual failures. It exits with code `5` when any attachment fails.

## Environment variables

| Variable | Purpose |
| --- | --- |
| `ASANA_ACCESS_TOKEN` | Temporary bearer token; overrides keyring. |
| `ASANA_API_BASE_URL` | Override API base URL for local development or a compatible proxy. |

## Scope

The CLI uses Asana personal access tokens and is intentionally read-only. OAuth, multiple profiles, writes, custom-field search, and caching are future work.
