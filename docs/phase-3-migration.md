# Phase 3 migration notes

Phase 3 is additive and ships as `asana` 0.5. Existing command grammar and JSON shapes remain unchanged.

New typed command families:

- `custom-field` and `custom-type`;
- `portfolio`;
- `goal` and `goal-relationship`;
- `project-portfolio-setting`;
- read-only `membership portfolio` alongside generic membership CRUD.

Canonical `story` commands already cover goal comments, and canonical `status-update` commands cover projects, portfolios, and goals. Phase 3 reuses these commands instead of introducing aliases with different JSON.

Custom-field values use repeatable `--custom-field GID=JSON_VALUE`. Values preserve JSON types; `null` explicitly clears a supported value. Advanced documented fields remain available through non-overlapping `--data`.

Portfolio duplication is asynchronous and returns a job. Goal metrics and supporting relationships are synchronous writes. Removing items, members, followers, settings, relationships, schemas, portfolios, or goals requires confirmation or `--yes`.

The public API currently exposes custom types as read-only and team custom-field settings as list-only. The CLI does not simulate unsupported mutations.
