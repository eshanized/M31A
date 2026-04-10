# Quick Start

## Installation

```bash
git clone https://github.com/eshanized/M31A.git
cd M31A
go build -o m31a ./cmd/m31a/
```

Or use Go install:

```bash
go install github.com/eshanized/M31A/cmd/m31a@latest
```

## First Run

```bash
./m31a
```

On first run, M31A checks for:
1. **Config file** — creates `~/.m31a/config.toml` if missing
2. **API keys** — prompts for key if not found in env/keychain/config
3. **Provider health** — verifies provider is reachable

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
Use the `/keychain set` command inside M31A to store the key securely.

## Basic Usage

1. Launch M31A: `./m31a`
2. Type your prompt at the `>` prompt
3. Press `Enter` to submit
4. View streaming response in real-time
5. Type follow-up prompts to continue the conversation

## Essential Commands

| Command | Description |
|---------|-------------|
| `/help` | Show help |
| `/model` | List/select models |
| `/session list` | View sessions |
| `/tools` | List/toggle tools |
| `/flush` | Clear screen |
| `/quit` | Exit |

## Next Steps

- Read **WORKFLOW.md** for the six-phase workflow
- Read **CONFIG.md** for configuration options
- Read **TOOLS.md** for available tools
- Read **KEYBINDINGS.md** for keyboard shortcuts
