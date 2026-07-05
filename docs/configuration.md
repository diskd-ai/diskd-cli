# Configuration and Authentication

`diskd` keeps local state under `$DISKD_HOME`, defaulting to `$HOME/.diskd`.

```text
$HOME/.diskd/
  config.yaml
  credentials
```

On Unix systems, the directory is created with `0700` permissions and the
credential file is written with `0600` permissions.

## Resolution Order

For non-secret settings:

```text
CLI flags -> environment -> config file -> built-in default
```

For credentials:

```text
APIS_ACCESS_TOKEN -> $DISKD_HOME/credentials
```

## Environment Variables

| Variable | Purpose |
| --- | --- |
| `APIS_BASE_URL` | API gateway base URL. |
| `APIS_ACCESS_TOKEN` | Bearer token used instead of stored credentials. |
| `DISKD_HOME` | Directory for config and credentials. |
| `DISKD_INSTALL_DIR` | Installer destination directory. |
| `DISKD_VERSION` | Installer release tag, for example `v0.1.2`. |
| `DISKD_NO_UPDATE_CHECK` | Disable startup update checks when set. |

## Config File

`config.yaml` stores non-secret settings only:

```yaml
base_url: https://apis.iosya.com
project: 01PROJECT
project_name: Project Name
output: json
```

The parser intentionally supports only simple `key: value` lines for the keys
used by the CLI.

## Credentials File

`credentials` stores JSON returned by `diskd login`:

```json
{
  "access_token": "...",
  "token_type": "Bearer"
}
```

Do not place tokens in `config.yaml`.

## Login with a Token

```sh
diskd login --token "$APIS_ACCESS_TOKEN"
```

This stores the token in `$DISKD_HOME/credentials`.

## Login with Client Credentials

```sh
diskd login --credentials-file ./credentials.json
```

Input schema:

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
issuer rejects the explicit Drive/project scopes, the CLI retries with the
client's default scopes.

## Identity Decoding

`whoami` decodes the JWT payload without signature verification for display
only. Workspace resolution follows the platform SDK order:

```text
ext.workspace_id -> workspace_id -> sub
```

The gateway remains responsible for token validation.

## Path Context

The selected project is local CLI context. It is not sent as a public Drive API
field. The CLI translates paths under the project root:

```text
project 01PROJECT + docs/a.txt -> /Projects/01PROJECT/docs/a.txt
workspace root + docs/a.txt -> /docs/a.txt
```

Use `diskd set-context --root` to clear the project context.

## Updates

`diskd update` installs the latest GitHub release for the current platform. It
downloads both the release archive and the `.sha256` file before replacing the
binary.

Human-facing commands perform a short startup update check and print a yellow
stderr notice when a newer version exists. These checks do not run for `--json`,
`--quiet`, or `diskd mcp serve`.

Disable the startup check:

```sh
export DISKD_NO_UPDATE_CHECK=1
```
