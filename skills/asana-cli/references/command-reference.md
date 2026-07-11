# Asana CLI command reference

Load this reference for exact commands, filters, pagination, JSON shapes, and error recovery. Put global flags before the command in generated examples for consistency.

## Common global flags

```text
asana --json [--workspace <workspace-gid>] [--verbose] <command>
```

- Always use `--json` for agent-driven calls.
- Use `--workspace` for one-command selection. Commands that operate directly on a task or attachment GID do not require it.
- Use `--verbose` only for diagnosis. Diagnostics go to stderr and secrets are redacted.

## Authentication and workspace discovery

```bash
asana --json auth status
asana --json workspace list --limit 50
asana --json workspace list --limit 50 --offset '<opaque-offset>'
```

`auth status` returns:

```json
{
  "user": { "gid": "...", "name": "...", "email": "..." },
  "token_source": "environment|keyring",
  "workspace_gid": "... or null"
}
```

These commands mutate local state and require explicit user intent:

```bash
asana auth login
asana --json auth logout
asana --json workspace use <workspace-gid>
```

## Projects

```bash
asana --json --workspace <gid> project list
asana --json --workspace <gid> project list --archived
asana --json --workspace <gid> project list --all-projects
asana --json --workspace <gid> project list --limit 100 --offset '<opaque-offset>'
```

The default excludes archived projects. `--archived` and `--all-projects` conflict.

## Task lists

```bash
# Authenticated user's open tasks in the workspace
asana --json --workspace <gid> task list

# Explicit assignee; accepts GID, email, or me
asana --json --workspace <gid> task list --assignee me

# Tasks in one project
asana --json task list --project <project-gid>

# Include completed tasks
asana --json task list --project <project-gid> --include-completed

# Manual pagination
asana --json --workspace <gid> task list --limit 100 --offset '<opaque-offset>'
```

Without `--project`, the default is `--assignee me` and incomplete tasks only. With `--project`, no assignee is implied. `--include-completed` expands results rather than returning only completed tasks.

## Task search

```bash
asana --json --workspace <gid> task search '<text>' [filters] --limit 50
```

Available filters:

```text
--assignee <gid|email|me>
--project <project-gid>
--completed | --incomplete
--due-before YYYY-MM-DD       --due-after YYYY-MM-DD
--created-before YYYY-MM-DD   --created-after YYYY-MM-DD
--modified-before YYYY-MM-DD  --modified-after YYYY-MM-DD
--sort relevance|due-date|created-at|completed-at|modified-at
--ascending                   requires --sort
--limit 1..100
```

Quote free-text queries as a single shell argument. Search covers task names and descriptions. It requires Asana Premium access and deliberately has no `--offset` because Asana search ordering is unstable. A response is limited to at most 100 results and must not be described as exhaustive when more may exist.

## Task details and comments

```bash
asana --json task get <task-gid>
asana --json task get <task-gid> --comments
asana --json task get <task-gid> --attachments
asana --json task get <task-gid> --comments --attachments
```

`task get` returns a task object directly. Requested relation arrays are added at the top level. The CLI internally fetches every page for these relation arrays.

Important task fields include `gid`, `name`, `completed`, `completed_at`, `assignee`, `due_on`, `due_at`, `start_on`, `notes`, `html_notes`, `created_at`, `modified_at`, `permalink_url`, and `memberships`. Prefer plain `notes` for summarization; use `html_notes` only when its formatting matters.

For paged access to human comments only:

```bash
asana --json task comments <task-gid> --limit 100
asana --json task comments <task-gid> --limit 100 --offset '<opaque-offset>'
```

Comment fields include `gid`, `created_at`, `created_by`, `text`, `html_text`, and `is_edited`. System activity is excluded. Because filtering occurs after retrieving an Asana story page, `data` may be empty even when `next_offset` is non-null.

## Attachments

List attachment metadata:

```bash
asana --json attachment list <task-gid> --limit 100
asana --json attachment list <task-gid> --limit 100 --offset '<opaque-offset>'
```

Download one known attachment:

```bash
asana --json attachment download <attachment-gid> --output '<path>'
```

Download every attachment on a task:

```bash
asana --json attachment download-all <task-gid> --directory '<directory>'
```

A successful single download returns metadata including its local `path` and byte count. `download-all` returns:

```json
{
  "task_gid": "123",
  "downloaded": [
    { "gid": "456", "name": "brief.pdf", "path": "downloads/brief.pdf", "bytes": 1024 }
  ],
  "failed": []
}
```

Attachment names are sanitized, signed URLs are refreshed before transfer, and existing files are not overwritten. A file hosted by a third party may lack a downloadable URL; report that failure without attempting to scrape the third-party site.

## Pagination algorithm

For `workspace list`, `project list`, `task list`, `task comments`, and `attachment list`:

1. Request the first page with the smallest useful `--limit` from 1 through 100.
2. Consume `data`.
3. If `next_offset` is null, stop.
4. Otherwise call the same command with `--offset` equal to the exact returned string.
5. Stop early when enough evidence exists for the user's request; fetch every page only when completeness is required.

Collection shape:

```json
{
  "data": [],
  "next_offset": "opaque-offset-or-null"
}
```

## Exit codes and recovery

With `--json`, stdout contains successful data and stderr contains errors:

```json
{
  "error": {
    "code": "api",
    "message": "Asana API error (403): Forbidden",
    "status": 403,
    "request_id": "optional-request-id"
  }
}
```

| Exit | Meaning | Agent action |
| --- | --- | --- |
| `0` | Success | Parse stdout. |
| `1` | Network, JSON parsing, or local I/O failure | Retry once only if plausibly transient; otherwise report it. |
| `2` | Invalid arguments or missing workspace configuration | Correct the command or discover/select a workspace. |
| `3` | Missing or rejected authentication | Ask the user to authenticate locally; never request the token in chat. |
| `4` | Asana API or permission error | Report status/message; narrow or change the query only when justified. |
| `5` | Some `download-all` items failed | Parse valid stdout summary and report both successes and failures. |

The client already retries rate limits and transient server failures. Do not create an unbounded retry loop. Preserve `request_id` when reporting API errors because it helps diagnosis.
