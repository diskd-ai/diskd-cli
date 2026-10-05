# Completion log

## Unreleased - 2026-10-05

### Fixes

- Login no longer retries silently without scopes when the token issuer
  answers `invalid_scope`. It now fails, names the rejected gateway scopes,
  and tells the user to re-fetch credentials with `diskd login` (or a fresh
  credentials file). Motivation: the silent retry produced tokens that carry no
  gateway route scopes, which would break once apis-service enforces
  `required_scopes`. Re-fetching re-registers the CLI client with the route
  scopes through iam-service.

## 0.2.0 - 2026-08-11

### Changes

- Added typed platform project list/get/create/update/delete commands so CLI
  users and agents no longer need a separate SDK client for project lifecycle.
- Added project-scoped Drive Session list/read/message-range/save/append/remove/
  rollback/delete commands using the `@diskd-ai/sdk` 6.1.1 contract. Public
  inputs remain project, session, and message IDs; the adapter derives Drive
  storage scope internally.
- Added streamed, atomic `download` to an explicit local destination while
  preserving raw `cat` output.
- Added root agent instructions and updated user, command, quick-start, and
  bundled skill references for the new CLI surface.

### Fixes

- Replaced whole-file upload and Drive-copy buffering with bounded streaming
  and aligned upload proxy URL handling with `/api/v1/drive/upload`.
- Replaced whole-response download buffering with direct writer streaming and
  surfaced HTTP, I/O, JSON-RPC, validation, flush, and persistence failures.
