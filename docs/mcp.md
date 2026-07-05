# MCP Server

`diskd mcp serve` runs an embedded MCP stdio server. It reuses the same config,
auth, path normalization, and Drive client as the CLI commands.

```sh
diskd mcp serve
```

When you run that command directly in a terminal, `diskd` prints a ready-to-copy
MCP server config for an LLM agent and exits. When an MCP client launches the
same command with stdio pipes, `diskd` starts the MCP protocol without printing
human instructions to stdout.

## Tools

The server exposes tool names aligned with the Drive MCP server:

| Tool | Drive method |
| --- | --- |
| `tools__ls` | `paths/tools/ls` |
| `tools__read` | `paths/tools/read` |
| `tools__glob` | `paths/tools/glob` |
| `tools__grep` | `paths/tools/grep` |
| `tools__vsearch` | `paths/tools/vsearch` |
| `tools__bi_query` | `paths/tools/bi-query` |

## Example Client Configuration

Use stored local credentials:

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

Authenticate before starting the LLM agent:

```sh
diskd login --token "$APIS_ACCESS_TOKEN"
```

Or put a token directly in the agent config:

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

Use `DISKD_HOME` in the `env` block when the agent process runs with a different
home directory:

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

## Protocol Notes

- The server accepts standard `Content-Length` framed JSON-RPC messages.
- It also accepts line-delimited JSON-RPC messages, which is useful for quick
  terminal checks.
- Notifications without `id` are ignored.
- Unknown JSON-RPC methods return `-32601`.

## Smoke Test

```sh
printf '{"jsonrpc":"2.0","id":1,"method":"initialize"}\n' | diskd mcp serve
```

List tools:

```sh
printf '{"jsonrpc":"2.0","id":1,"method":"tools/list"}\n' | diskd mcp serve
```

Call a search tool:

```sh
printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"tools__grep","arguments":{"query":"hello","paths":["docs"]}}}\n' | diskd mcp serve
```
