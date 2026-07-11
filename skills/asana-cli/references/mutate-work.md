# Mutate daily Asana work

Use typed commands in this reference before considering `asana api`. Always add `--json` for agent calls.

## Tasks

Create:

```bash
asana --json --workspace <workspace-gid> task create \
  --name '<name>' \
  [--assignee <gid|email|me>] \
  [--due-on YYYY-MM-DD | --due-at <timestamp>] \
  [--start-on YYYY-MM-DD | --start-at <timestamp>] \
  [--project <gid>] [--section <gid>] \
  [--parent <task-gid>] [--follower <user-gid>] [--tag <tag-gid>] \
  [--notes '<text>' | --notes-file <path|->] \
  [--html-notes '<body>...</body>' | --html-notes-file <path|->] \
  [--custom-field GID=JSON_VALUE] [--data <JSON|@file|->]
```

`--section` requires exactly one `--project`. Use repeated flags for projects, followers, tags, and custom fields. `--data` provides advanced official fields; it cannot overlap a typed field.

Custom-field values use JSON semantics: quote enum-option/user GIDs (`GID=\"OPTION_GID\"`), leave numbers unquoted, use `true`/`false` for booleans, and use JSON arrays where the Asana field accepts multiple values.

Update only requested fields:

```bash
asana --json task update <task-gid> \
  [--expect-modified-at <timestamp>] \
  [--name '<name>'] [--assignee <gid|email|me> | --clear-assignee] \
  [--due-on YYYY-MM-DD | --due-at <timestamp> | --clear-due] \
  [--start-on YYYY-MM-DD | --start-at <timestamp> | --clear-start] \
  [--complete | --incomplete] \
  [--notes '<text>' | --notes-file <path|-> | --clear-notes] \
  [--set-custom-field GID=JSON_VALUE] [--clear-custom-field <gid>] \
  [--data <JSON|@file|->]
```

Delete only after re-reading and explicit authorization:

```bash
asana --json --dry-run task delete <task-gid>
asana --json --yes task delete <task-gid>
```

Duplicate and inspect its asynchronous job:

```bash
asana --json task duplicate <task-gid> [--name '<copy name>'] [--include field,...]
asana --json job wait <job-gid> --timeout 10m
```

Get by a workspace custom ID:

```bash
asana --json --workspace <gid> task get-custom '<custom-id>'
```

## Comments and stories

```bash
asana --json task comment list <task-gid>
asana --json task comment add <task-gid> --text '<comment>'
asana --json task comment add <task-gid> --text-file <path|->
asana --json task comment update <story-gid> --text '<new text>'
asana --json --yes task comment delete <story-gid>
```

Use `--html-text '<body>...</body>'` for intentional rich text. A story GID, not task GID, identifies a comment for update/delete. Users may delete only stories they created.

Canonical story commands also support task and goal comments:

```bash
asana --json story list --task <gid>
asana --json story list --goal <gid>
asana --json story create --task <gid> --text '<comment>'
asana --json story create --goal <gid> --text '<comment>'
asana --json story get <story-gid>
asana --json story update <story-gid> --pin
asana --json --yes story delete <story-gid>
```

## Tags

```bash
asana --json tag list
asana --json tag get <tag-gid>
asana --json tag create --name '<name>' [--color <color>]
asana --json tag update <tag-gid> [--name '<name>'] [--color <color>]
asana --json --yes tag delete <tag-gid>

asana --json task tag list <task-gid>
asana --json task tag add <task-gid> <tag-gid>
asana --json --yes task tag remove <task-gid> <tag-gid>
asana --json task list --tag <tag-gid>
asana --json --workspace <gid> task search '<text>' --tag <tag-gid>
```

Resolve tag names through `typeahead tag '<query>'`; never assume a name is unique.

## Sections and project placement

```bash
asana --json section create --project <project-gid> --name '<name>'
asana --json section get <section-gid>
asana --json section update <section-gid> --name '<name>'
asana --json section move <section-gid> --project <project-gid> \
  (--before-section <gid> | --after-section <gid>)
asana --json section task-add <section-gid> --task <task-gid>
asana --json --yes section delete <section-gid>
```

Sections must be empty before deletion. Moving a task to a section removes it from the other section in that project.

Project membership and positioning:

```bash
asana --json task project list <task-gid>
asana --json task project add <task-gid> <project-gid> [--section <gid>] \
  [--insert-before <task-gid> | --insert-after <task-gid> | --at-start | --at-end]
asana --json --yes task project remove <task-gid> <project-gid>
```

`--at-start` and `--at-end` require `--section`.

## Subtasks and relationships

```bash
asana --json task subtask list <task-gid>
asana --json task subtask create <task-gid> --name '<name>'
asana --json task parent set <task-gid> --parent <parent-gid>
asana --json task parent clear <task-gid>

asana --json task dependency list <task-gid>
asana --json task dependency add <task-gid> <dependency-gid>...
asana --json --yes task dependency remove <task-gid> <dependency-gid>...
asana --json task dependent list <task-gid>
asana --json task dependent add <task-gid> <dependent-gid>...
asana --json --yes task dependent remove <task-gid> <dependent-gid>...

asana --json task follower add <task-gid> <user-gid>...
asana --json --yes task follower remove <task-gid> <user-gid>...
```

Asana limits combined task dependencies and dependents. Report API limit errors without silently dropping requested relationships.

## Attachments

```bash
asana --json attachment get <attachment-gid>
asana --json attachment list <parent-gid>
asana --json attachment upload --parent <task|project|brief-gid> --file <path>
asana --json attachment attach-url --parent <gid> --name '<name>' --url '<url>'
asana --json --yes attachment delete <attachment-gid>
```

Do not retry an ambiguous upload. List attachments on the parent first to determine whether it succeeded.

## Resolution and advanced search

```bash
asana --json --workspace <gid> typeahead task '<query>' --count 20
asana --json --workspace <gid> typeahead user '<query>'
asana --json --workspace <gid> typeahead project '<query>'
asana --json --workspace <gid> typeahead tag '<query>'
```

Typeahead is non-exhaustive and cannot be paginated. Use exact GIDs after resolution.

Advanced task search supports:

```bash
--section <gid> --tag <gid> --follower <gid>
--custom-field FIELD_GID.OPERATOR=VALUE
--custom-type-status CUSTOM_TYPE_GID=STATUS_OPTION_GID
--filter OFFICIAL_KEY=VALUE
```

Use `--filter` only with a verified official Asana search parameter. Search is eventually consistent; never use it to verify an immediate mutation.
