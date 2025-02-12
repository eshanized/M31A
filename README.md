# M31A

A terminal-based AI coding assistant written in Go.

## Overview

M31A runs a six-phase spec-driven workflow engine (Initialize → Discuss → Plan → Execute → Verify → Ship), communicates exclusively through OpenRouter and OpenCode Zen gateways, renders a Gemini-caliber TUI via the Charm library stack, and ships as a single static binary under MIT license with no telemetry.

## Quick Start

```bash
go build -o m31a ./cmd/m31a
./m31a
```

## License

MIT — no vendor lock-in, no telemetry, no paid tiers.
