# diskd MCP Server

`diskd mcp serve` runs an embedded MCP stdio server. It reuses the same config,
auth, path normalization, and Drive client as the CLI commands.

```sh
diskd mcp serve
```

Run directly in a terminal, `diskd` prints a ready-to-copy agent config and
exits. When an MCP client launches the same command with stdio pipes, `diskd`
speaks the MCP protocol and prints no human instructions to stdout.

## Tools

Tool names are aligned with the Drive MCP server:

| Tool | Drive method |
| --- | --- |
| `tools__ls` | `paths/tools/ls` |
| `tools__read` | `paths/tools/read` |
| `tools__glob` | `paths/tools/glob` |
| `tools__grep` | `paths/tools/grep` |
| `tools__vsearch` | `paths/tools/vsearch` |
| `tools__bi_query` | `paths/tools/bi-query` |

## Client configuration

Authenticate first (stored local credentials), then point the client at the
binary:

```sh
diskd login --token "$APIS_ACCESS_TOKEN"
```

```json
{
  "mcpServers": {
    "diskd": {
      "command": "diskd",
      "args": ["mcp", "serve"],
      "env": {
        "APIS_BASE_URL": "https://apis.iosya.com"
      }
    }
  }
}
```

Provide the token inline instead of pre-authenticating:

```json
{
  "mcpServers": {
    "diskd": {
      "command": "diskd",
      "args": ["mcp", "serve"],
      "env": {
        "APIS_BASE_URL": "https://apis.iosya.com",
        "APIS_ACCESS_TOKEN": "..."
      }
    }
  }
}
```

Set `DISKD_HOME` in `env` when the agent process runs under a different home
directory than where you logged in:

```json
{
  "mcpServers": {
    "diskd": {
      "command": "diskd",
      "args": ["mcp", "serve"],
      "env": {
        "DISKD_HOME": "/home/user/.diskd"
      }
    }
  }
}
```

## Protocol notes

- Accepts standard `Content-Length` framed JSON-RPC messages.
- Also accepts line-delimited JSON-RPC messages (handy for terminal checks).
- Notifications without `id` are ignored.
- Unknown JSON-RPC methods return `-32601`.

## Smoke tests

```sh
# Initialize
printf '{"jsonrpc":"2.0","id":1,"method":"initialize"}\n' | diskd mcp serve

# List tools
printf '{"jsonrpc":"2.0","id":1,"method":"tools/list"}\n' | diskd mcp serve

# Call a search tool
printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"tools__grep","arguments":{"query":"hello","paths":["docs"]}}}\n' | diskd mcp serve
```
