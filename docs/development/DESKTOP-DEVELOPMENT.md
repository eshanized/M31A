# M31A Desktop Development Guide

This guide covers building, testing, and developing the M31A Desktop application (`apps/desktop`).

---

## 1. Prerequisites

- **Rust**: 1.80+ (`rustup default stable`)
- **Node.js**: 20+ and npm 10+
- **Linux System Libraries** (for WebKit2GTK / Tauri 2):
  ```bash
  sudo apt-get install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

---

## 2. Directory Structure

```text
apps/desktop/
├── src/                      # React 18 + TypeScript frontend
│   ├── app/                  # Main App and Layout shell
│   ├── features/             # Feature components (chat, approvals, models, etc.)
│   ├── lib/                  # IPC client and event bridge
│   ├── stores/               # AppContext & unified state
│   ├── styles/               # Theme and base CSS
│   ├── test/                 # Vitest test suites and setup
│   └── types/                # Strongly-typed IPC DTOs & event unions
├── src-tauri/                # Rust desktop host crate (`m31a-desktop`)
│   ├── capabilities/         # Tauri 2 permission capabilities
│   ├── icons/                # Multi-platform app icons
│   ├── src/                  # IPC commands, security, state, lifecycle
│   ├── Cargo.toml            # Desktop crate manifest
│   └── tauri.conf.json       # Tauri 2 configuration
├── package.json              # Frontend scripts & dependencies
├── tsconfig.json             # TypeScript configuration
└── vite.config.ts            # Vite & Vitest configuration
```

---

## 3. Development Workflow

### Frontend Development
To run the Vite dev server with hot module replacement:
```bash
cd apps/desktop
npm install
npm run dev
```

### Full Desktop Application (Rust Host + React Frontend)
To run the complete desktop application:
```bash
# In apps/desktop
npm run build
cargo run -p m31a-desktop
```

---

## 4. Testing & Verification

### Frontend Tests (Vitest)
```bash
cd apps/desktop
npm run typecheck
npm run test
```

### Rust Desktop Tests
```bash
cargo test -p m31a-desktop
```

### Workspace Quality Checks
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```
