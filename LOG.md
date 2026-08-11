# Completion log

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
