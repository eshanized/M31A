# Quick Start

## Installation

```bash
# macOS (Homebrew)
brew install eshanized/tap/m31a

# Windows (Scoop)
scoop install m31a

# Linux packages (.deb, .rpm, .apk)
# Download from https://github.com/eshanized/M31A/releases

# Linux / macOS (one-liner)
curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/master/install.sh | bash

# From source (any OS)
git clone https://github.com/eshanized/M31A.git
cd M31A
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
```

## First Run

```bash
./m31a
```

On first run, M31 Autonomous:
1. Creates `~/.m31a/config.toml` if missing
2. Prompts for API key if not found in env/keychain/config
3. Verifies provider is reachable

## Set Up API Key

### Option 1: Environment Variable
```bash
export M31A_OPENROUTER_API_KEY="sk-or-v1-..."
./m31a
```

### Option 2: Config File
```toml
# ~/.m31a/config.toml
[provider.openrouter]
api_key = "sk-or-v1-..."
```

### Option 3: Keychain (recommended)
Use the `/keychain set` command inside M31 Autonomous to store the key securely.

## Basic Usage

1. Launch M31 Autonomous: `./m31a`
2. Type your prompt at the `>` prompt
3. Press `Enter` to submit
4. View streaming response in real-time
5. Type follow-up prompts to continue the conversation

## Headless Mode

```bash
# Single prompt (no TUI)
m31a --prompt "What files are in the project?"

# Full workflow execution
m31a --goal "Create a REST API with authentication and database"
```

## Essential Commands

| Command | Description |
|---------|-------------|
| `/help` | Show help |
| `/model` | List/select models |
| `/sessions` | View sessions |
| `/tools` | List/toggle tools |
| `/flush` | Clear screen |
| `/quit` | Exit |

## Next Steps

- Read **[WORKFLOW.md](WORKFLOW.md)** for the seven-phase workflow
- Read **[CONFIG.md](CONFIG.md)** for configuration options
- Read **[TOOLS.md](TOOLS.md)** for available tools
- Read **[KEYBINDINGS.md](KEYBINDINGS.md)** for keyboard shortcuts
