# Authentication, Configuration, and Path Context

## Contents

- [State directory](#state-directory)
- [Resolution order](#resolution-order)
- [Environment variables](#environment-variables)
- [Config file](#config-file)
- [Credentials file](#credentials-file)
- [Login with a token](#login-with-a-token)
- [Login with client credentials](#login-with-client-credentials)
- [Identity decoding](#identity-decoding)
- [Path context](#path-context)
- [Updates](#updates)

## State directory

`diskd` keeps local state under `$DISKD_HOME`, defaulting to `$HOME/.diskd`.

```text
$HOME/.diskd/
  config.yaml
  credentials
```

On Unix, the directory is created `0700` and the credential file `0600`.

## Resolution order

Non-secret settings:

```text
CLI flags -> environment -> config file -> built-in default
```

Credentials:

```text
APIS_ACCESS_TOKEN -> $DISKD_HOME/credentials
```

## Environment variables

| Variable | Purpose |
| --- | --- |
| `APIS_BASE_URL` | API gateway base URL. |
| `APIS_ACCESS_TOKEN` | Bearer token used instead of stored credentials. |
| `DISKD_HOME` | Directory for config and credentials. |
| `DISKD_INSTALL_DIR` | Installer destination directory. |
| `DISKD_VERSION` | Installer release tag, e.g. `v0.1.2`. |
| `DISKD_NO_UPDATE_CHECK` | Disable startup update checks when set. |

## Config file

`config.yaml` stores non-secret settings only. The parser supports simple
`key: value` lines for the keys the CLI uses:

```yaml
base_url: https://apis.iosya.com
project: 01PROJECT
project_name: Project Name
output: json
```

Never put tokens in `config.yaml`.

## Credentials file

`$DISKD_HOME/credentials` stores JSON returned by `diskd login`:

```json
{
  "access_token": "...",
  "token_type": "Bearer"
}
```

## Login with a token

```sh
diskd login --token "$APIS_ACCESS_TOKEN"
```

Stores the token in `$DISKD_HOME/credentials`.

## Login with client credentials

```sh
diskd login --credentials-file ./credentials.json
```

Input schema (the file you provide):

```json
{
  "issuer": "https://auth.example",
  "clientId": "client-id",
  "clientSecret": "client-secret",
  "audience": "diskd-api",
  "apisUrl": "https://apis.iosya.com"
}
```

The CLI fetches OIDC discovery from `issuer`, calls the token endpoint with
`grant_type=client_credentials`, and stores the returned access token. If the
issuer rejects the explicit Drive/project scopes, it retries with the client's
default scopes. `apisUrl` is written as the gateway base URL for future
commands, so a credential-file login is a convenient way to persist the gateway.

## Identity decoding

`whoami` decodes the JWT payload without signature verification, for display
only. Workspace resolution follows the platform SDK order:

```text
ext.workspace_id -> workspace_id -> sub
```

Token validation remains the gateway's responsibility.

## Path context

The selected project is **local CLI context** and is never sent as a public
Drive API field. The CLI translates relative paths under the project root:

```text
project 01PROJECT + docs/a.txt   -> /Projects/01PROJECT/docs/a.txt
workspace root + docs/a.txt      -> /docs/a.txt
```

`.` and `..` segments are rejected before any network call. Use
`diskd set-context --root` to clear the project context.

## Updates

`diskd update` installs the latest GitHub release for the current platform,
downloading both the archive and its `.sha256` before replacing the binary.

Human-facing commands run a short startup update check and print a yellow stderr
notice when a newer version exists. The check does not run for `--json`,
`--quiet`, or `diskd mcp serve`. Disable it for a session:

```sh
export DISKD_NO_UPDATE_CHECK=1
```
