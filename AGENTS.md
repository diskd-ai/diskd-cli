# diskd CLI agent instructions

Use `diskd` for authenticated project, session, and Drive operations through the
public APIS gateway. Prefer `--json` for automation and place every global flag
before the subcommand.

## Safe command sequence

```sh
diskd --json whoami
diskd --json project list
diskd set-context <project-id>
diskd --json get-context
```

Use canonical project and session IDs in agent workflows. Never construct or
pass Drive session storage paths, inodes, or `.sessions` filenames. Session
commands derive `/Projects/<project-id>` from `--project` or the saved context.

## Projects

```sh
diskd --json project list
diskd --json project get <project-id>
diskd --json project create <name> [--description <text>] [--icon <value>] [--icon-color <value>]
diskd --json project update <project-id> [--name <name>] [--description <text>] [--icon <value>] [--icon-color <value>]
diskd --json project delete <project-id> --yes
```

Project deletion is permanent. Run it only when the user explicitly authorized
that exact project mutation.

## Sessions

Read-only discovery should be the default agent flow:

```sh
diskd --project <project-id> --json session list
diskd --project <project-id> --json session read <session-id>
diskd --project <project-id> --json session read <session-id> --limit 20
diskd --project <project-id> --json session messages <session-id> --limit 20 [--before <message-id>]
```

Session write operations accept typed SDK-compatible JSON files:

```sh
diskd --project <project-id> --json session save <session-document.json> [--attribute <value>...]
diskd --project <project-id> --json session append <session-id> <messages.json>
diskd --project <project-id> --json session remove <session-id> <message-id>...
diskd --project <project-id> --json session rollback <session-id> <after-message-id>
diskd --project <project-id> --json session delete <session-id> --yes
```

`session remove`, `rollback`, `save`, `append`, and `delete` mutate persisted
Drive session state. Do not run them during inspection-only work.

## Drive transfers

```sh
diskd --project <project-id> upload <local-path> --dest <drive-dir> [--recursive] [--force]
diskd --project <project-id> cat <drive-path> > <local-path>
diskd --project <project-id> --json download <drive-path> <local-path> [--version <n>] [--force]
```

Upload and download bodies are streamed. `download` writes through a temporary
file and only persists the destination after a complete response and flush.
Without `--force`, it refuses to replace an existing file.

## Agent behavior

- Treat non-zero exit status and stderr as failures; never infer success from a
  partial JSON document.
- Never print or persist bearer tokens outside the CLI credential store.
- Use `--json` for parsed output; `cat` is raw bytes and must not be parsed as
  JSON.
- Ask before destructive project/session commands or `--force` replacement.
- For development, run the smallest affected `cargo test` filter. At feature
  completion run `cargo clippy --workspace -- -D warnings`.
