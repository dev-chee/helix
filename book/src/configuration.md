# Configuration

To override global configuration parameters, create a `config.toml` file located in your config directory:

- Linux and Mac: `~/.config/helix/config.toml`
- Windows: `%AppData%\helix\config.toml`

> 💡 You can easily open the config file by typing `:config-open` within Helix normal mode.

Example config:

```toml
theme = "onedark"

[editor]
line-number = "relative"
mouse = false

[editor.cursor-shape]
insert = "bar"
normal = "block"
select = "underline"

[editor.file-picker]
hidden = false

# AI Agent configuration (ACP)
[agent]
claude = { command = "claude", args = ["--mcp"], timeout = 120 }
iflow = { command = "iflow", args = ["--experimental-acp"], transport = "newline-delimited", timeout = 120 }
```

## Agent Configuration

Helix supports AI coding agents through the Agent Client Protocol (ACP). You can configure agents in your `config.toml`:

```toml
[agent]
# Example: Claude Code agent
claude = { command = "claude", args = ["--mcp"], timeout = 120 }

# Example: Agent using newline-delimited JSON (e.g., iflow)
iflow = { command = "iflow", args = ["--experimental-acp"], transport = "newline-delimited", timeout = 120 }

# Example: Custom agent with environment variables
my-agent = { command = "my-agent-cli", args = ["--stdio"], enabled = false, environment = { API_KEY = "secret" } }
```

### Agent Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enabled` | bool | `true` | Whether this agent is enabled |
| `command` | string | (required) | Command to run the agent |
| `args` | [string] | `[]` | Arguments to pass to the agent |
| `environment` | map | `{}` | Environment variables for the agent |
| `config` | value | `null` | Configuration to pass to the agent |
| `timeout` | int | `60` | Request timeout in seconds |
| `transport` | string | `"content-length"` | Message format: `"content-length"` (standard) or `"newline-delimited"` |

### Transport Types

- `content-length` (default): Standard JSON-RPC over stdio format with `Content-Length` headers. Most ACP implementations use this.
- `newline-delimited`: Each JSON message is terminated by a newline. Use this for agents like `iflow` that don't support the standard format.

### Agent Commands

| Command | Description |
|---------|-------------|
| `:agent-start <name>` | Start an AI agent |
| `:agent-stop [name]` | Stop an agent (all if no name given) |
| `:agent-list` | List running agents |
| `:agent-prompt <message>` | Send a prompt to an agent |
| `:agent-cancel` | Cancel current session |

You can use a custom configuration file by specifying it with the `-c` or
`--config` command line argument, for example `hx -c path/to/custom-config.toml`.
You can reload the config file by issuing the `:config-reload` command. Alternatively, on Unix operating systems, you can reload it by sending the USR1
signal to the Helix process, such as by using the command `pkill -USR1 hx`.

Finally, you can have a `config.toml` and a `languages.toml` local to a project by putting it under a `.helix` directory in your repository.
Its settings will be merged with the configuration directory and the built-in configuration.

