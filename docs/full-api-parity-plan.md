# Full Asana REST API parity plan

Status: Phase 0 foundation and typed Phase 1 daily workflows implemented; later resource phases pending  
Target: `asana` 1.0  
API baseline: public Asana REST API catalog as of 2026-07-11

## 1. Definition of parity

`asana` reaches full parity when every public Asana REST API endpoint is represented in a versioned support manifest as one of:

1. a stable, typed CLI command;
2. an intentionally raw-only endpoint callable through `asana api` while a typed command is pending; or
3. an explicit exclusion with a reason, such as an App Components callback implemented by an integration server rather than called by a local client.

Parity means parity with the public REST API, not every feature visible in the Asana web UI. The API itself does not expose every UI operation, and some resources are read-only or support only part of CRUD. The CLI must expose exactly the operations the API supports and must not simulate unsupported mutations.

The initial 0.1 read-only commands remain backward-compatible through 1.x unless a security issue requires a break.

## 2. Product principles

- Keep the default output pleasant for humans and `--json` deterministic for agents.
- Prefer short, typed commands for common work and retain a complete raw API escape hatch.
- Treat GIDs as identities. Names are selectors only when the command can prove they resolve uniquely.
- Make partial updates patch-like: send only fields explicitly supplied by the caller.
- Never lose rich-text content by reading and rewriting a representation the CLI cannot preserve.
- Make destructive operations difficult to trigger accidentally but straightforward to automate intentionally.
- Preserve read-your-write behavior by fetching the mutated resource directly; do not verify writes through eventually consistent search.
- Do not retry ambiguous writes automatically. Retry safe reads and only writes known to be safe to repeat.
- Keep API-plan limitations, permissions, required scopes, and enterprise/service-account requirements visible in errors.
- Build the agent contract from the same command metadata as the CLI so documentation does not drift.

## 3. Command grammar

Use noun-first commands consistently:

```text
asana [global-options] <resource> <verb> [resource-options]
```

Common verbs:

```text
list  search  get  create  update  delete
add   remove  move  duplicate  instantiate  export
```

Relationship operations add one nested noun where it improves clarity:

```bash
asana task tag add <task-gid> <tag-gid>
asana task project add <task-gid> <project-gid> --section <gid>
asana task follower add <task-gid> <user-gid>...
asana task dependency add <task-gid> <dependency-gid>...
asana portfolio item add <portfolio-gid> <item-gid>
```

High-frequency aliases may coexist with canonical resource commands:

```bash
asana task comment add <task-gid> --text "Ready for review"
asana story create --task <task-gid> --text "Ready for review"
```

The agent contract uses one canonical spelling only. Human-friendly aliases are documented separately and return identical JSON.

## 4. Common input and output contract

### Global options

```text
--profile <name>             select credentials/config profile
--workspace <gid>            override profile workspace
--json                       stable machine output
--fields <path,...>          override default response fields
--limit <1..100>             page size
--offset <opaque>            explicit next-page cursor
--all                        consume all pages deliberately
--max-items <n>              cap --all and protect against runaway reads
--verbose                    redacted diagnostics on stderr
--request-timeout <duration> per-request timeout
```

Keep `--json` as the compatibility alias for `--output json`. A later table/TSV output option may be added without changing JSON.

### Mutation input

Common fields use typed flags. Long and advanced input supports files and stdin:

```bash
asana task create --name "Fix login" --project 123 --section 456
asana task update 789 --due-on 2026-08-01 --complete
asana task update 789 --notes-file ./description.md
asana task comment add 789 --text-file -
asana task update 789 --data @patch.json
```

Rules:

- `--data @file` reads a JSON object; `--data -` reads stdin.
- Typed flags and `--data` may be combined only when their fields do not overlap.
- Plain and rich text flags conflict: for example `--notes`/`--notes-file` versus `--html-notes`/`--html-notes-file`.
- Clear operations are explicit: `--clear-due`, `--clear-assignee`, `--clear-custom-field <gid>`.
- Boolean updates use semantic pairs such as `--complete`/`--incomplete`, never an ambiguous optional boolean.
- Repeated relationship flags are additive only when named `--add-*`; replacement flags are named `--set-*`.
- Dates and date-times are validated locally before sending a request.
- JSON numbers stay numbers; GIDs stay strings.

### JSON shapes

Keep the current resource and collection contract:

```json
{
  "gid": "123",
  "name": "Example"
}
```

```json
{
  "data": [],
  "next_offset": null
}
```

Mutation commands return the affected resource when Asana returns one. Empty API responses use an explicit operation receipt:

```json
{
  "ok": true,
  "operation": "task.tag.add",
  "resource_gid": "123",
  "related_gid": "456"
}
```

Delete receipts include `deleted: true`. Async operations return a job object and optionally support `--wait` with bounded polling:

```bash
asana project duplicate <gid> --name "Copy" --wait --timeout 10m
asana job wait <job-gid> --timeout 10m
```

## 5. Mutation safety

Classify operations by risk:

| Class | Examples | Interactive behavior | Non-interactive behavior |
| --- | --- | --- | --- |
| Read | get, list, search | execute | execute |
| Additive write | create task, add comment/tag/follower | execute and summarize | execute only from explicit caller intent |
| Reversible update | rename, assign, set due date, complete | show changed fields | execute only from explicit caller intent |
| Destructive | delete, remove membership, remove user, reject request | prompt with resource identity | require `--yes` |
| High-impact admin | workspace user removal, role changes, export, bulk/batch writes | prompt with impact summary | require `--yes` and explicit profile/workspace |

Every mutation supports `--dry-run`. Dry-run prints the normalized method, redacted URL, body, affected GIDs, risk class, and required confirmation without sending the request.

For updates, support `--expect-modified-at <timestamp>` where the resource exposes `modified_at`. The CLI performs a preflight read and refuses a stale update. This is best-effort conflict detection because the Asana API does not provide a general atomic compare-and-swap contract.

Never retry `POST`, `PUT`, or `DELETE` after a timeout or lost response unless that exact operation is documented and implemented as safely repeatable. Return an `ambiguous_write` error telling the caller to inspect the target before retrying. Continue honoring `Retry-After` for reads.

Batch commands are not transactions. A batch response can have HTTP 200 while individual actions fail, and execution order is not guaranteed. JSON must expose per-action status and use a dedicated partial-failure exit code.

## 6. Authentication and profiles

Replace the single implicit profile with named profiles:

```bash
asana auth login --profile work
asana auth status --profile work
asana config set workspace 123 --profile work
asana profile list
asana profile use work
```

Support:

- PAT in OS keyring;
- service-account tokens in OS keyring or process environment;
- OAuth authorization-code flow as a later 1.x authentication provider;
- environment token override for CI, with an optional profile-specific variable mapping;
- displayed token source and discovered OAuth scopes without ever displaying a token.

The CLI must map 401, 402, 403, and missing OAuth scopes separately. Enterprise and add-on failures should name the capability involved when the API response makes that knowable.

Admin resources require an explicit profile and workspace even if defaults exist. This prevents accidental operations against the wrong organization.

## 7. Client architecture

The current endpoint-specific blocking client is appropriate for the MVP but will not scale to the full API. Keep synchronous `reqwest::blocking`, but split responsibilities:

```text
src/
  api/
    client.rs          generic GET/POST/PUT/DELETE/multipart transport
    request.rs         data/options envelopes and query encoding
    response.rs        envelopes, errors, headers, request IDs
    pagination.rs      cursor and bounded --all iteration
    retry.rs           method-aware retry policy
  commands/
    task.rs
    project.rs
    ...                one module per user-facing resource family
  domain/
    task.rs
    custom_field.rs
    ...                typed request/response models
  input.rs             --data, @file, stdin, clear/set semantics
  output.rs            human renderer and stable JSON
  safety.rs            risk classes, dry-run, confirmation
  config.rs            profiles and workspace defaults
  parity.rs            support-manifest access
```

Create a library crate (`src/lib.rs`) and leave `main.rs` as parsing plus exit-code wiring. This makes the command layer reusable by future shells, plugins, or an MCP adapter without spawning the binary.

Use typed request structs for stable/high-frequency endpoints. Preserve unknown response fields with flattened JSON maps where forward compatibility matters. Do not deserialize the entire API into one universal model.

### Raw API escape hatch

Provide complete immediate coverage through:

```bash
asana api get /tasks/123 --query opt_fields=name,notes
asana api post /tasks --data @task.json
asana api put /tasks/123 --data @patch.json
asana api delete /tasks/123 --yes
asana api upload /attachments --field parent=123 --file ./brief.pdf
```

Guardrails:

- accept only relative paths under the configured Asana API base by default;
- reject path traversal and arbitrary hosts;
- apply the same auth, output, error, retry, dry-run, and confirmation policies as typed commands;
- require `--allow-unknown-endpoint` if the path is absent from the vendored support manifest;
- never allow arbitrary authorization headers from arguments;
- classify non-GET raw calls as writes and raw DELETE as destructive.

`asana api` is not a replacement for typed commands. It closes zero-day gaps when Asana adds an endpoint and enables advanced fields before the ergonomic wrapper is released.

## 8. Resource coverage map

The support manifest must track every endpoint, request parameter/body field, scope, plan restriction, command, response type, pagination mode, mutation risk, documentation link, and implementation status. A typed endpoint is complete only when every currently documented input has either a typed CLI representation or a documented `--data`/`--query` route; merely calling the endpoint is not option parity.

### Everyday work management

| Resource | Required CLI coverage |
| --- | --- |
| Tasks | list by workspace/assignee/project/section/tag/user-task-list; get by GID/custom ID; full search including tags, sections, followers, dates, completion, custom fields, custom types and sort; create/update/delete/duplicate |
| Subtasks | list/create; set, change, clear, and position parent |
| Task relationships | dependencies/dependents list/add/remove; project membership add/remove/reorder; followers add/remove |
| Stories/comments | get/list; create comments on tasks/goals; update/delete own comments; pin supported story types |
| Tags | list/search/get/create/update/delete; list task tags; add/remove tag on task; task list/search by tag |
| Attachments | list/get/download/download-all; upload file; attach external URL; delete; support task/project/project-brief parents |
| Sections | list/get/create/rename/delete/reorder; add/move/reorder task within section |
| User task lists | get list identity and tasks |
| Typeahead | fast non-exhaustive lookup of users, tasks, projects, portfolios, tags, templates, goals, teams, agents and actors |
| Reactions | list supported reactions; expose mutation only if/when the public API adds it |

Example task UX:

```bash
asana task create --name "Investigate timeout" --project 123 --section 456 \
  --assignee me --due-on 2026-07-15 --tag 789 --notes-file ./notes.md

asana task update 111 --name "Investigate API timeout" --clear-due \
  --set-custom-field 222=333 --complete

asana task search timeout --tag 789 --section 456 --incomplete \
  --custom-field 222=333 --sort modified-at

asana task comment add 111 --text-file ./comment.md
asana task delete 111 --yes
```

### Projects and collaboration

| Resource | Required CLI coverage |
| --- | --- |
| Projects | list/get/search/create/update/delete/duplicate; team/workspace creation; task counts; save as template |
| Project relationships | members/followers add/remove; custom fields add/remove; task/project listing |
| Project briefs | create/get/update/delete with plain and rich text |
| Status updates | list/get/create/delete for projects, portfolios, and goals; do not invent update because the API does not support it |
| Project templates | list/get/delete/instantiate and create from project; job waiting |
| Task templates | list/get/delete/instantiate; job waiting |
| Users | list/get/favorites/update where authorized; workspace/team-scoped lookup |
| Teams | list/get/create/update; users add/remove; team projects/templates |
| Team/workspace/project/portfolio memberships | all get/list/create/update/delete operations exposed by current APIs |
| Workspaces | list/get/update name; invite/remove users with high-impact confirmation; workspace events |
| Access requests | list/create/approve/reject with high-impact confirmation for decisions |

### Structured data and strategic work

| Resource | Required CLI coverage |
| --- | --- |
| Custom fields | list/get/create/update/delete for text, enum, multi-enum, number, date, people; enum option create/update/reorder; task/project/goal/portfolio value operations |
| Custom field settings | list and attach/remove for project, portfolio, team, and goal where supported |
| Custom types | list/get and task search by custom type/status; expose only API-supported mutations |
| Portfolios | full CRUD, duplicate, item add/remove, member add/remove, custom fields, statuses |
| Goals | full CRUD, metrics create/update, followers, parent goals, custom fields, comments and statuses |
| Goal relationships | get/list/add/update/remove supporting work |
| Project portfolio settings | get/list/update |
| Allocations | full CRUD/list by project/user/placeholder |
| Rates | full CRUD/list by project and resource |
| Budgets | full CRUD/list by parent |

### Time, automation, and administration

| Resource | Required CLI coverage |
| --- | --- |
| Time tracking entries | list/get/create/update/delete; filter by task/project/portfolio/user |
| Time tracking categories | full CRUD and related entry listing |
| Timesheet approval statuses | list/get/create/update state transitions |
| Time periods | read/list only, matching API capability |
| OOO entries | full CRUD/list for user |
| Roles | full CRUD/list with admin confirmation |
| Rules | trigger incoming-web-request rules |
| Jobs | get/wait with bounded polling and terminal-state exit codes |
| Events | initial sync token plus polling for resource/workspace; persist cursor only when explicitly requested |
| Webhooks | list/get/create/update/delete; creation documents external HTTPS handshake requirement; CLI does not pretend localhost can satisfy it |
| Batch | up to 10 independent actions, per-action results, no transaction semantics, no upload/export/SCIM nesting |
| Graph/resource/org exports | create, inspect job/export state, bounded wait, safe download of expiring result URL |
| Audit log | enterprise service-account read stream with its special cursor semantics and 90-day retention warning |
| AI Studio usage/seats | service-account read commands with explicit licensing errors |
| Agents | workspace list/get for Asana AI teammates |

### Explicitly separate surfaces

- SCIM uses a distinct administrative API and authentication model. Plan it as `asana admin scim ...`, not as ordinary workspace CRUD.
- App Components Lookups, Modal Forms, Rule Actions, and Widgets are callbacks implemented by an integration server. Record them as excluded from invocation parity. A future `asana app-component scaffold` would be a separate product surface.
- Deprecated project-status endpoints are not exposed as new canonical commands; use status updates. Raw access remains possible while Asana still serves them.
- Features unavailable through the public API remain out of scope and are named in the manifest rather than approximated through browser automation.

## 9. Delivery phases

Each phase ends with updated command help, JSON contract, parity manifest, agent reference, migration notes, and live sandbox smoke evidence for the endpoints added in that phase.

### Phase 0 — scalable foundation

- Refactor into library, generic transport, domain command modules, and method-aware retry.
- Add request bodies, PUT/POST/DELETE, multipart uploads, typed errors for 402/scope/ambiguous writes.
- Add `--data`, stdin/file input, `--fields`, `--dry-run`, confirmations, profiles, `--all` with caps, and job waiting.
- Add `asana api` and the vendored endpoint support manifest.
- Version the JSON contract and exit-code registry.

Exit criterion: any catalogued REST endpoint is safely callable through `asana api`, even before its typed command exists.

### Phase 1 — complete daily task workflows

- Task CRUD and rich text.
- Comments/story get/create/update/delete.
- Tags CRUD, task tag relationships, and list/search filtering by tag.
- Attachments upload/external/delete plus current downloads.
- Subtasks, parent operations, dependencies/dependents, followers, project membership, and positioning.
- Sections CRUD/reorder and task positioning.
- User/project/tag/typeahead resolution helpers.
- Full task search filters, including custom fields and section/tag/custom-type filters.

Exit criterion: a user or agent can triage, create, assign, schedule, classify, discuss, move, complete, reopen, and delete work without raw API calls.

### Phase 2 — projects, people, and templates

- Project CRUD/search/duplicate, relationships, brief, task counts, and status updates.
- Project and task templates plus async jobs.
- Users, favorites, teams, workspace/team/project memberships, and workspace operations.
- Access requests.

Exit criterion: ordinary project administration and collaboration are covered by typed commands.

### Phase 3 — portfolios, goals, and custom data

- Custom fields/options/settings and typed value encoding.
- Portfolios/items/members/statuses.
- Goals/metrics/relationships/followers/comments/statuses.
- Generic memberships and project-portfolio settings.

Exit criterion: strategic work and custom schema workflows do not require raw API calls.

### Phase 4 — resource management and enterprise

- Time entries/categories/timesheets, allocations, rates, budgets, OOO.
- Roles, exports, audit log, AI Studio usage, and agents.
- Service-account/profile guardrails and licensing diagnostics.

Exit criterion: every non-integration-server endpoint has a typed command unless the manifest records a reviewed raw-only exception.

### Phase 5 — events, webhooks, batches, and drift control

- Event polling and explicit cursor persistence.
- Webhook management and handshake-aware guidance.
- Batch execution and partial-result reporting.
- Automated comparison of the vendored API catalog with the latest official catalog/OpenAPI snapshot.
- Release gate that rejects undocumented new or removed endpoints.

Exit criterion: the parity manifest contains no unexplained gaps and API drift is detected before release.

## 10. Agent skill design

Split the skill by workflow so agents load only relevant instructions:

```text
skills/asana-cli/
  SKILL.md
  references/
    read-work.md
    mutate-work.md
    projects-and-people.md
    custom-fields.md
    strategic-work.md
    admin-and-enterprise.md
    command-reference.md
    json-and-errors.md
```

The skill must teach agents to:

1. resolve workspace and resource identity before mutation;
2. inspect the current resource immediately before updating or deleting it;
3. distinguish explicit user authorization from merely discussing a possible change;
4. use `--dry-run` when the mutation is broad, ambiguous, destructive, or admin-level;
5. use typed commands before `asana api`;
6. never use raw writes to bypass confirmation policy;
7. avoid search for immediate write verification because search is eventually consistent;
8. pass long content through stdin/files and preserve rich text exactly;
9. handle partial batch/download results instead of treating HTTP 200 as universal success;
10. report affected resource names, GIDs, direct URLs, changed fields, and unresolved ambiguity.

Mutation output should be compact enough for agents to inspect before subsequent actions. Add optional `before`, `after`, and `changed_fields` metadata under an agent-oriented envelope without changing the default single-resource JSON contract, for example through `--json-details`.

Generate the detailed command reference and risk table from command metadata. Hand-write only decision policy and workflow guidance.

## 11. Verification strategy

The original MVP intentionally had no automated tests. Full write parity changes the risk profile: destructive and partial-update behavior cannot be considered complete based only on compilation and help output.

Required verification layers:

- request-shape tests against a local HTTP server for every typed endpoint wrapper;
- golden tests for stable JSON success/error/partial-result shapes;
- parser tests for conflicts, clear/set semantics, stdin/files, confirmation, and dry-run;
- redaction tests ensuring tokens, signed URLs, and secrets never enter diagnostics;
- retry tests proving ambiguous writes are not repeated;
- pagination and async-job state-machine tests;
- manifest coverage checks mapping every catalogued endpoint to a command/status;
- opt-in live smoke suite against a dedicated Asana developer sandbox;
- destructive live tests only against resources created by that smoke run and tagged with a unique run identifier.

Do not run live writes against a user's normal workspace in CI. A release may proceed with unavailable enterprise-only smoke tests only when their request-shape tests pass and the unverified live capability is declared in release evidence.

## 12. Exit codes

Preserve current codes and extend them without reusing meanings:

| Code | Meaning |
| --- | --- |
| 0 | complete success |
| 1 | network, parsing, or local I/O failure |
| 2 | CLI usage or configuration error |
| 3 | authentication or required-scope failure |
| 4 | Asana API/permission/plan error |
| 5 | partial attachment download failure |
| 6 | destructive operation not confirmed |
| 7 | stale precondition or ambiguous write outcome |
| 8 | partial batch/bulk failure |
| 9 | async job failed or timed out |

JSON errors retain `code`, `message`, `status`, and `request_id`, and add optional `details`, `required_scope`, `required_plan`, `retry_after`, and `ambiguous` fields.

## 13. Completion checklist

The 1.0 parity claim requires evidence for every item:

- every current public REST endpoint appears in the support manifest;
- every endpoint is typed, raw-callable, or explicitly excluded with a reviewed reason;
- all daily task workflows use typed commands;
- tag and custom-field filtering work in list/search flows where the API supports them;
- all supported CRUD operations have correct patch, clear, confirmation, and response semantics;
- human output and JSON are documented for every command family;
- agent references contain no stale command or safety guidance;
- help, generated docs, and manifest agree;
- automated local verification passes;
- supported live sandbox smoke suites pass;
- no secret appears in stdout, stderr, dry-run, verbose logs, or saved config;
- API catalog drift check is clean for the release baseline.

## 14. First implementation slice

Begin with Phase 0 plus the smallest end-to-end Phase 1 vertical slice:

1. generic JSON write transport and mutation error model;
2. `--dry-run`, confirmation, `--data`, and file/stdin text input;
3. `task create`, `task update`, and `task delete`;
4. `task comment add/update/delete`;
5. `tag list/get/create/update/delete` and `task tag add/remove`;
6. `task list --tag` and full search tag filters;
7. request-shape validation and live sandbox smoke instructions;
8. corresponding skill mutation policy and command reference.

This slice proves the architecture on the workflows users and agents need most, while the raw command makes the remaining official API reachable under the same safety policy.
