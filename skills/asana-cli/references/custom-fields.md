# Work with Asana custom fields

Load this reference when reading or changing custom-field schemas, enum options, settings, custom types, or values. Use `--json` for agent calls.

## Schema and options

```bash
asana --json --workspace <gid> custom-field list
asana --json custom-field get <field-gid>
asana --json --workspace <gid> custom-field create --name '<name>' --type <text|enum|multi_enum|number|date|people|reference>
asana --json custom-field update <field-gid> [--name '<name>'] [--description '<text>']
asana --json --yes custom-field delete <field-gid>

asana --json custom-field enum-option create --custom-field <field-gid> --name '<name>' [--color <color>]
asana --json custom-field enum-option update <option-gid> [--name '<name>'] [--enabled false]
asana --json custom-field enum-option move <option-gid> --custom-field <field-gid> \
  (--before-enum-option <gid>|--after-enum-option <gid>)
```

Use `--data <JSON|@file|->` for type-specific creation fields such as initial enum options, number formatting, custom labels, reference restrictions, or other documented fields without typed flags. Do not overlap typed and raw fields.

There is no enum-option delete endpoint. Disable an option with `--enabled false`; do not delete or rewrite the whole custom field to imitate deletion.

## Settings and custom types

```bash
asana --json custom-field settings project <project-gid>
asana --json custom-field settings portfolio <portfolio-gid>
asana --json custom-field settings goal <goal-gid>
asana --json custom-field settings team <team-gid>

asana --json custom-type list --project <project-gid>
asana --json custom-type get <custom-type-gid>
```

Attach/remove settings with the parent command:

```bash
asana --json project custom-field <project-gid> <field-gid>
asana --json portfolio custom-field <portfolio-gid> <field-gid>
asana --json goal custom-field <goal-gid> <field-gid>
asana --json --yes portfolio custom-field <portfolio-gid> <field-gid> --remove
```

Team settings are read-only in the current public API. Do not invent attach/remove operations.

## Values

Set values with repeatable `--custom-field GID=JSON_VALUE` on task, project, portfolio, and goal create/update commands where supported. Preserve JSON types:

- enum, user, and reference GIDs: JSON strings;
- number values: JSON numbers;
- multi-enum, people, and multi-reference values: JSON arrays;
- dates: API-documented date objects or strings for that resource;
- clearing: JSON `null`.

Example:

```bash
asana --json goal update <goal-gid> \
  --custom-field 'FIELD_A="OPTION_GID"' \
  --custom-field 'FIELD_B=42' \
  --custom-field 'FIELD_C=null'
```

Read the field schema before encoding a value. Never infer an enum option from its display name when names are duplicated or disabled.
