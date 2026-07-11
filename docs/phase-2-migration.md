# Phase 2 migration notes

Phase 2 is additive and ships as `asana` 0.4. Existing task, section, attachment, tag, story, raw API, profile, and job commands keep their prior grammar and JSON shapes.

New typed command families are:

- expanded `project` and `workspace` verbs;
- `project-template` and `task-template`;
- `status-update`;
- `user` and `team`;
- `membership`, including generic membership CRUD and legacy read-only membership views;
- `access-request`.

Project creation is scoped through the selected workspace or `--team`. Advanced official fields remain available through non-overlapping `--data`. Project duplication, project-template instantiation, task-template instantiation, and save-as-template return an asynchronous job; use `job wait` before consuming the result.

Operations that can remove or grant organizational access retain destructive confirmation. Team/workspace user changes and access-request approval/rejection additionally require explicit `--profile` and `--workspace`; a configured default is intentionally insufficient for these high-impact commands.

The public API does not expose delete-team or mutation operations for the legacy `project_memberships`, `team_memberships`, and `workspace_memberships` resources. The CLI does not simulate them. Use typed generic `membership` CRUD where the API supports the requested parent/member combination.

Deprecated project-status endpoints remain available through guarded `asana api`; new automation should use typed `status-update` commands.
