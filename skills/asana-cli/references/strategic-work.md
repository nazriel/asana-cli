# Manage strategic work

Load this reference for portfolios, goals, metrics, supporting work, statuses, comments, and project–portfolio settings.

## Portfolios

```bash
asana --json --workspace <gid> portfolio list [--owner me]
asana --json portfolio get <portfolio-gid>
asana --json --workspace <gid> portfolio create --name '<name>' [--privacy-setting members_only]
asana --json portfolio update <gid> [--name '<name>'] [--archive|--unarchive]
asana --json portfolio duplicate <gid> --name '<copy>' [--include members,child_projects]
asana --json --yes portfolio delete <gid>
```

Duplicate returns an asynchronous job. Wait with `asana --json job wait <job-gid>` before using `new_portfolio.gid`.

Items, members, and custom fields:

```bash
asana --json portfolio item <portfolio-gid>
asana --json portfolio item <portfolio-gid> --add <project|portfolio-gid> [--insert-before <gid>|--insert-after <gid>]
asana --json --yes portfolio item <portfolio-gid> --remove <item-gid>
asana --json portfolio member <portfolio-gid> --add <user-gid>[,<gid>...]
asana --json --yes portfolio member <portfolio-gid> --remove <user-gid>[,<gid>...]
asana --json portfolio custom-fields <portfolio-gid>
```

Use `status-update list|create|delete --parent <portfolio-gid>` for portfolio statuses. The API does not support editing status updates.

## Goals

```bash
asana --json --workspace <gid> goal list [--team <gid>] [--portfolio <gid>] [--time-periods <gid,...>]
asana --json goal get <goal-gid>
asana --json --workspace <gid> goal create --name '<name>' --owner <user-gid>
asana --json goal update <goal-gid> [--name '<name>'] [--status <status>]
asana --json --yes goal delete <goal-gid>
```

Use explicit clear flags for nullable goal fields. Use `story list|create --goal <gid>` for comments and `status-update ... --parent <goal-gid>` for status history.

Metrics and followers:

```bash
asana --json goal metric set <goal-gid> --unit <none|currency|percentage> --initial 0 --target 100 [--current 25]
asana --json goal metric update <goal-gid> --current 50
asana --json goal follower <goal-gid> --add <user-gid>[,<gid>...]
asana --json --yes goal follower <goal-gid> --remove <user-gid>[,<gid>...]
asana --json goal parents <goal-gid>
```

## Supporting work

```bash
asana --json goal-relationship list --supported-goal <goal-gid>
asana --json goal-relationship get <relationship-gid>
asana --json goal-relationship add --goal <goal-gid> --supporting-resource <goal|project|task|portfolio-gid> [--contribution-weight 0.5]
asana --json goal-relationship update <relationship-gid> --contribution-weight 0.75
asana --json --yes goal-relationship remove --goal <goal-gid> --supporting-resource <gid>
```

Contribution weights must be between 0 and 1. Read the relationship immediately before changing or removing it.

## Project–portfolio settings

```bash
asana --json project-portfolio-setting list --project <project-gid>
asana --json project-portfolio-setting list --portfolio <portfolio-gid>
asana --json project-portfolio-setting get <setting-gid>
asana --json project-portfolio-setting update <setting-gid> (--inherit-access|--no-inherit-access)
```

Changing inherited access can alter who can reach a project. Inspect portfolio/project membership first and preview with `--dry-run` when impact is not obvious.

Report resource names and GIDs, changed metrics/access relationships, and asynchronous job GIDs. Verify directly through get/list commands rather than search.
