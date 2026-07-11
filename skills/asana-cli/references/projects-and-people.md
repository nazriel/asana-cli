# Manage projects, people, and templates

Load this reference for project administration, collaboration, templates, memberships, workspace changes, and access decisions. Use `--json` for every agent call.

## Safety sequence

1. Resolve the workspace, project, team, and user GIDs.
2. Read the current object immediately before changing or deleting it.
3. Preview changes affecting access, membership, or multiple people with `--dry-run`.
4. Require explicit user authorization before adding/removing users, deleting resources, or approving/rejecting access.
5. For high-impact user/team/workspace operations, pass both `--profile <name>` and `--workspace <gid>`; the CLI rejects implicit administrative context.
6. Execute once. If a write is ambiguous, inspect the resource or async job before deciding whether to retry.

## Projects

```bash
asana --json --workspace <workspace-gid> project list
asana --json project list --team <team-gid>
asana --json project list --task <task-gid>
asana --json --workspace <workspace-gid> project search '<text>' [--team <gid>]
asana --json project get <project-gid>
asana --json --workspace <workspace-gid> project create --name '<name>' [--team <gid>]
asana --json project update <project-gid> [--name '<name>'] [--archive|--unarchive]
asana --json --yes project delete <project-gid>
asana --json project duplicate <project-gid> --name '<copy>' [--include task_notes,task_dates]
asana --json project task-counts <project-gid>
```

Use `--data <JSON|@file|->` for documented fields without a typed flag. Typed and raw fields may not overlap. Wait for duplicate/template jobs with `asana --json job wait <job-gid>`.

Project relationships:

```bash
asana --json project member <project-gid> --add <user-gid>[,<user-gid>...]
asana --json --yes project member <project-gid> --remove <user-gid>[,<user-gid>...]
asana --json project follower <project-gid> --add <user-gid>[,<user-gid>...]
asana --json project custom-fields <project-gid>
asana --json project custom-field <project-gid> <field-gid> [--important]
asana --json --yes project custom-field <project-gid> <field-gid> --remove
```

## Briefs and status updates

```bash
asana --json project brief get <project-gid>
asana --json project brief create --project <gid> --title '<title>' (--text '<text>'|--html-text '<body>...</body>')
asana --json project brief update <brief-gid> [--title '<title>'] [--text '<text>']
asana --json --yes project brief delete <brief-gid>

asana --json status-update list --parent <project|portfolio|goal-gid>
asana --json status-update get <status-gid>
asana --json status-update create --parent <gid> --status-type <type> --text '<text>'
asana --json --yes status-update delete <status-gid>
```

Do not invent status-update editing: the public API supports get/list/create/delete, not update.

## Templates

```bash
asana --json --workspace <gid> project-template list [--team <gid>]
asana --json project-template get <template-gid>
asana --json project-template instantiate <template-gid> --name '<name>' [--requested-date GID=YYYY-MM-DD]
asana --json --yes project-template delete <template-gid>
asana --json --workspace <gid> project save-as-template <project-gid> --name '<name>' (--public|--private) [--team <gid>]

asana --json task-template list --project <project-gid>
asana --json task-template get <template-gid>
asana --json task-template instantiate <template-gid> [--name '<name>']
asana --json --yes task-template delete <template-gid>
```

Instantiation and project duplication return jobs. Use `job wait`; do not assume the resulting resource exists before the job succeeds.

## Users, teams, and workspaces

```bash
asana --json --workspace <gid> user list
asana --json user list --team <team-gid>
asana --json user get <user-gid>
asana --json --workspace <gid> user get <user-gid> --in-workspace
asana --json --workspace <gid> user favorites <user-gid> [--resource-type project]

asana --json --workspace <gid> team list
asana --json team list --user <user-gid>
asana --json team get <team-gid>
asana --json --workspace <gid> team create --name '<name>'
asana --json team update <team-gid> --name '<name>'
```

Administrative membership changes require explicit context and confirmation:

```bash
asana --json --profile <profile> --workspace <gid> --dry-run team user-add <team-gid> --user <user-gid>
asana --json --profile <profile> --workspace <gid> --yes team user-remove <team-gid> --user <user-gid>
asana --json --profile <profile> --workspace <gid> --yes workspace user-remove <workspace-gid> --user <user-gid|email>
```

## Memberships and access requests

Use generic memberships for current project/portfolio/goal sharing and access levels:

```bash
asana --json membership list [--parent <gid>] [--member <gid>]
asana --json membership get <membership-gid>
asana --json membership create --parent <gid> --member <user|team-gid> [--access-level editor]
asana --json membership update <membership-gid> --access-level <level>
asana --json --yes membership delete <membership-gid>
```

Use `membership project|team|workspace` for the legacy read-only membership resources. Do not simulate mutations that those endpoints do not expose.

```bash
asana --json access-request list --target <project|portfolio-gid>
asana --json access-request create --target <gid> [--message '<context>']
asana --json --profile <profile> --workspace <gid> --yes access-request approve <request-gid>
asana --json --profile <profile> --workspace <gid> --yes access-request reject <request-gid>
```

Report the affected resource name, GID, changed relationship/access level, and job GID when asynchronous. Verify direct resource or membership endpoints, never search.
