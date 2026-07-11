# Raw API and mutation safety

Use this reference only when the required operation has no typed CLI command. A raw call is still subject to the same authorization, identity, and safety requirements as a typed command.

## Before a mutation

1. Confirm that the user explicitly requested the concrete change.
2. Resolve names to unique GIDs. Do not write when multiple resources plausibly match.
3. Read the current target directly by GID.
4. Confirm the official Asana method, relative path, request fields, and plan/scope requirements.
5. Run the exact mutation with `--dry-run` when it is destructive, broad, administrative, or has unclear impact.
6. Execute once. Verify success by direct `get`, not by eventually consistent search.

## Profiles

```bash
asana --json profile list
asana --profile work --json auth status
asana --profile work --json workspace use <workspace-gid>
```

Use `--profile` when surrounding context identifies a non-default account. Never change the active profile merely to perform one command.

## Raw reads

```bash
asana --json api get /tasks/<gid> --query opt_fields=gid,name,notes
asana --json --all --max-items 1000 api get /tags --query workspace=<gid>
```

Provide query parameters with repeated `--query KEY=VALUE`. Keep the path relative; never include a host, query string, fragment, or `..` segment.

## Raw JSON writes

`--data` is the contents of Asana's `data` object. The CLI adds the envelope.

```bash
asana --json --dry-run api post /tasks --data '{"name":"Example","workspace":"123"}'
asana --json api post /tasks --data @task.json
asana --json api put /tasks/<gid> --data @patch.json
asana --json api post /tasks/<gid>/stories --data -
```

Use stdin or `@file` for long content. Send only fields intended to change in PUT requests. The CLI adds global `--fields` as Asana response options.

## Uploads

```bash
asana --json api upload /attachments \
  --file ./brief.pdf \
  --field parent=<task-gid>
```

Use repeated `--field KEY=VALUE` for multipart text fields. Do not retry an upload after an ambiguous network outcome until attachment metadata has been inspected.

## Deletes

```bash
asana --json --dry-run api delete /tasks/<gid>
asana --json --yes api delete /tasks/<gid>
```

Raw DELETE is destructive. Non-interactive execution requires `--yes`. Re-read the target immediately before deletion and report the exact name and GID to the user.

## Unknown paths

The vendored catalog rejects unknown top-level API paths. `--allow-unknown-endpoint` is for newly released official endpoints, not for guessing. Use it only after verifying the path in current official Asana documentation.

## Ambiguous outcomes

The CLI never retries raw writes after a transport failure. Exit code `7` and error code `ambiguous_write` mean the request may have reached Asana. Do not repeat it automatically. Inspect the intended resource, comments, relationships, or attachments first.

## Dry-run output

Dry-run does not require authentication and returns the normalized method, path, query, data, risk, and confirmation requirement. It never sends a network request.
