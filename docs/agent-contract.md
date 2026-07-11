# Agent contract

Contract version: 2

This document is the stable integration surface for an AI skill or a script using `asana`.

## Safety and authentication

- Prefer typed commands. `asana api` can perform raw writes when no typed command exists.
- Perform a mutation only from explicit user intent and read the current target first.
- Use `--dry-run` for destructive, broad, ambiguous, or administrative mutations.
- Never automatically retry an `ambiguous_write` result.
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
| List sections in a project | `asana section list --project <project-gid> --json` |
| List the user's open tasks | `asana task list --json` |
| List open tasks in a section | `asana task list --section <section-gid> --json` |
| Search task title and description | `asana task search <query> --json` |
| Read a task description | `asana task get <task-gid> --json` |
| Read task plus comments/files | `asana task get <task-gid> --comments --attachments --json` |
| List human comments | `asana task comments <task-gid> --json` |
| List attachments | `asana attachment list <parent-gid> --json` |
| Download one attachment | `asana attachment download <attachment-gid> --json` |
| Download all task attachments | `asana attachment download-all <task-gid> --directory <path> --json` |
| List profiles | `asana profile list --json` |
| Raw REST read | `asana api get /relative/path --query key=value --json` |
| Preview a raw write | `asana api post /relative/path --data @file --dry-run --json` |
| Execute a raw write | `asana api post\|put /relative/path --data @file --json` |
| Delete through raw API | `asana api delete /relative/path --yes --json` |
| Upload multipart file | `asana api upload /attachments --file <path> --field parent=<gid> --json` |
| Inspect or wait for job | `asana job get\|wait <job-gid> --json` |
| Create task | `asana task create --name <name> [options] --json` |
| Update task | `asana task update <gid> [changes] --expect-modified-at <timestamp> --json` |
| Delete task | `asana task delete <gid> --yes --json` |
| Add/update/delete comment | `asana task comment add\|update\|delete ... --json` |
| List/create/update/delete tag | `asana tag list\|create\|update\|delete ... --json` |
| Add/remove task tag | `asana task tag add\|remove <task-gid> <tag-gid> --json` |
| Upload/delete attachment | `asana attachment upload\|delete ... --json` |
| Resolve object by name | `asana typeahead <type> <query> --json` |

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
| `6` | Destructive operation was not confirmed |
| `7` | A write may or may not have reached Asana; inspect before retrying |
| `9` | An asynchronous job failed or timed out |

## Operational notes

- List commands default to 50 items; use `--limit` from 1 through 100.
- `task list` without `--project` or `--section` means the authenticated user's open tasks in the selected workspace.
- `task list --section <gid>` uses Asana's section endpoint directly. Discover section GIDs with `section list --project <gid>`; do not fetch every task merely to inspect `memberships`.
- `task search` has no `--offset`: Asana says its search ordering is not stable. Limit it to the smallest useful result set.
- Comments are stories whose `resource_subtype` is `comment_added`; system activity is excluded.
- Attachment filenames supplied by Asana are sanitized. Existing destination files are never overwritten.
- `--all` consumes cursors until completion or `--max-items` (default 10,000) is reached. A non-null `next_offset` means the cap stopped collection.
- Raw paths must be relative and catalogued. `--allow-unknown-endpoint` is only for a newly documented official endpoint.
- JSON passed to raw POST/PUT is the inner `data` object; the CLI creates the Asana envelope.
- Typed task updates are patches: pass only requested changes. Use `--expect-modified-at` from a direct prior `task get` when available.
- Typed destructive commands require interactive confirmation or `--yes`; agents use `--yes` only for explicitly authorized changes.
- Verify mutations through direct resource or relationship reads. Task search is eventually consistent.
