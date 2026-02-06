# palingenesis

[![CI](https://github.com/Jack-R-Hong/palingenesis/actions/workflows/ci.yaml/badge.svg)](https://github.com/Jack-R-Hong/palingenesis/actions/workflows/ci.yaml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Agent resurrection system for continuous AI workflow execution.

## Overview

A lightweight Rust daemon that monitors AI coding assistant sessions (opencode/Sisyphus) and automatically resumes work when the agent stops due to rate limits, context limits, or other interruptions.

## Features

- **Automatic Detection**: Monitors opencode sessions for stop signals
- **Smart Classification**: Distinguishes rate limits vs context exhaustion vs user exit
- **Intelligent Waiting**: Respects `Retry-After` headers, uses exponential backoff
- **Session Resumption**: Continues same session or starts new from `Next-step.md`
- **CLI Control**: Full daemon management via command line
- **Notifications**: Webhook, Discord, Slack, ntfy.sh support
- **Observability**: OpenTelemetry traces export, Prometheus metrics endpoint

## Installation

### From Source

```bash
cargo install --path .
```

### Requirements

- Rust 1.85+ (edition 2024)
- Linux (Ubuntu 20.04+, Fedora 38+) or macOS (12.0+ Monterey)

## Usage

```bash
# Start the daemon
palingenesis daemon start

# Check status
palingenesis status

# View logs
palingenesis logs --follow

# Stop the daemon
palingenesis daemon stop
```

## OpenCode MCP Integration

palingenesis can run as a local MCP server for OpenCode.

### Setup

1. Generate the OpenCode MCP configuration snippet:
   ```bash
   palingenesis mcp config
   ```

2. Add the output to your OpenCode config file:
   - Linux/macOS: `~/.config/opencode/opencode.json`

Example configuration:

```json
{
  "mcpServers": {
    "palingenesis": {
      "type": "local",
      "command": ["palingenesis", "mcp", "serve"],
      "enabled": true
    }
  }
}
```

## Configuration

Configuration file location:
- Linux: `~/.config/palingenesis/config.toml`
- macOS: `~/Library/Application Support/palingenesis/config.toml`

```bash
# Initialize config
palingenesis config init
M
# Validate config
palingenesis config validate
```

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run with coverage
cargo llvm-cov nextest

# Check formatting
cargo fmt -- --check

# Run linter
cargo clippy
```

## License

MIT License - see [LICENSE](LICENSE) for details.
