# Agent contract

This document is the stable integration surface for an AI skill or a script using `asana`.

## Safety and authentication

- The MVP has no write commands.
- Authenticate with an OS-keyring token via `asana auth login`, or set `ASANA_ACCESS_TOKEN` for the current process.
- Do not print, log, or pass the token as a command argument.
- Pass `--json` for every agent-driven invocation.

## Commands

| Intent | Command |
| --- | --- |
| Identify the active account | `asana auth status --json` |
| Find accessible workspaces | `asana workspace list --json` |
| Select a persistent default workspace | `asana workspace use <gid> --json` |
| List projects | `asana project list --json` |
| List the user's open tasks | `asana task list --json` |
| Search task title and description | `asana task search <query> --json` |
| Read a task description | `asana task get <task-gid> --json` |
| Read task plus comments/files | `asana task get <task-gid> --comments --attachments --json` |
| List human comments | `asana task comments <task-gid> --json` |
| List attachments | `asana attachment list <task-gid> --json` |
| Download one attachment | `asana attachment download <attachment-gid> --json` |
| Download all task attachments | `asana attachment download-all <task-gid> --directory <path> --json` |

Every command accepts `--workspace <workspace-gid>` as a one-command override when the command needs a workspace.

## JSON

Single-resource commands output the resource directly:

```json
{
  "gid": "123",
  "name": "Prepare release notes"
}
```

Collection commands output this envelope:

```json
{
  "data": [
    { "gid": "123", "name": "Prepare release notes" }
  ],
  "next_offset": "opaque-offset"
}
```

Use the `next_offset` verbatim with `--offset`; do not try to parse it. When it is `null`, there is no next page.

`task get --comments --attachments` returns the task fields at the top level and only adds the requested relation fields:

```json
{
  "gid": "123",
  "name": "Prepare release notes",
  "notes": "...",
  "comments": [],
  "attachments": []
}
```

`attachment download-all` always writes a summary to stdout, even when some files fail:

```json
{
  "task_gid": "123",
  "downloaded": [
    { "gid": "456", "name": "brief.pdf", "path": "downloads/brief.pdf", "bytes": 1024 }
  ],
  "failed": []
}
```

## Errors and exit codes

Errors are JSON on stderr when `--json` is active and never contaminate stdout:

```json
{
  "error": {
    "code": "api",
    "message": "Asana API error (403): Forbidden",
    "status": 403,
    "request_id": "optional-asana-request-id"
  }
}
```

| Code | Meaning |
| --- | --- |
| `0` | Success |
| `1` | Network, parsing, or local I/O failure |
| `2` | Invalid CLI use or missing configuration |
| `3` | Missing or rejected authentication |
| `4` | Asana API error, including permissions and Premium-only search |
| `5` | Partial `attachment download-all` failure |

## Operational notes

- List commands default to 50 items; use `--limit` from 1 through 100.
- `task list` without `--project` means the authenticated user's open tasks in the selected workspace.
- `task search` has no `--offset`: Asana says its search ordering is not stable. Limit it to the smallest useful result set.
- Comments are stories whose `resource_subtype` is `comment_added`; system activity is excluded.
- Attachment filenames supplied by Asana are sanitized. Existing destination files are never overwritten.
