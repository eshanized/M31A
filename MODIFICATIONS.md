# Modifications (Apache-2.0 §4(d))

M31A is an MIT-licensed derivative work (Author: Eshan Roy,
https://github.com/eshanized/M31A) based on **gemini-cli** (Apache-2.0, ©
Google LLC). This file records material changes, as required by Apache-2.0.
Original Google copyright and license notices are retained in
`LICENSE-APACHE-2.0`, `NOTICE`, and source `@license` headers; M31A itself is
licensed under the MIT License in `LICENSE`. M31A is not affiliated with or
endorsed by Google or NVIDIA.

## Fork basis

- Source: `github.com/google-gemini/gemini-cli`, `0.73.0-nightly.20261123`,
  Apache-2.0.
- Method: full source copy, then rename + substantive modification (not
  clean-room).

## Renaming

- Project `Gemini CLI` → **M31A**; root package `@google/gemini-cli` → `m31a`.
- Scope `@google/gemini-cli*` → `@m31a/*`; CLI `gemini`→`m31a`; bundle
  `gemini.js`→`m31a.js`.
- Config dir `.gemini`→`.m31a`; `.geminiignore`→`.m31aignore`;
  `GEMINI.md`→`M31A.md`.
- App env prefix `GEMINI_CLI_*`/`GEMINI_*`(app)→`M31A_*`;
  `GEMINI_DIR`→`M31A_DIR`.
- Internal `m31a-*`, repo URL `eshanized/M31A`, author `Eshan Roy`, license
  `MIT`. (Cosmetic `Gemini*` identifiers and Gemini _model_ names may persist
  pending Phase-3 removal.)

## Model / backend changes

- **NVIDIA NIM single provider** `nvidia-nemotron`; `NvidiaNimContentGenerator`
  (OpenAI-compatible `https://integrate.api.nvidia.com/v1`, `NVIDIA_API_KEY`,
  default `nvidia/nemotron-3-ultra-550b-a55b`). `AuthType.USE_NVIDIA_NIM`.
- Removal of Google sign-in, Vertex, Code Assist, Gemini models, `@google/genai`
  runtime in progress (see notes); `@google/genai` currently retained type-only.
- New default model set to NVIDIA NIM (replacing Gemini defaults).

## Removed / disabled (Phase 3 in progress)

- Google sign-in (OAuth personal / Cloud Shell / ADC / Vertex) — target:
  removed.
- Google telemetry: Clearcut + `@google-cloud/*` telemetry exporters + Google
  usage-stats headers — target: removed; **OpenTelemetry OTLP kept**.
- Google sandbox images / `@google-cloud` deps — target: removed.
- `@m31a` sandbox `sandboxImageUri` config removed.

## Known residual (not yet removed)

- `@google/genai` type-only imports; `AuthType` deprecated members; Gemini model
  constants; `Gemini*` symbols; `@google-cloud` packages; `packages/*` not yet
  slimmed.
- Rename-damaged tests/snapshots/`memory-tests` fixtures pending repair.

## Tooling changes

- `.npmrc`: removed `@google:registry=wombat-dressing-room`; `allow-remote=all`.
- npm v12: install-scripts approval for `esbuild`, `node-pty`, etc.
- `@google/genai` retained **type-only** (structural `ContentGenerator`
  contract).
