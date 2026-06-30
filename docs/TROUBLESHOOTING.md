# Troubleshooting

Common issues and their solutions.

---

## "Invalid API key" Error

**Problem:** Provider rejects the API key.

**Solutions:**
1. Check the environment variable is set:
   ```bash
   echo $M31A_OPENROUTER_API_KEY
   ```
2. Verify the key in `~/.m31a/config.toml` is correct
3. Re-enter via keychain: `/keychain set` inside M31 Autonomous
4. Ensure no trailing whitespace in the key value

---

## "No models available" Error

**Problem:** Provider returns no models or list fails.

**Solutions:**
1. Check provider health by running M31 Autonomous and checking the header status indicator
2. Verify network connectivity to OpenRouter, Zen, or Nvidia
3. Increase model cache TTL if rate-limited:
   ```toml
   [features]
   model_cache_ttl_minutes = 15
   ```
4. Check if API key has model access permissions

---

## "Config file not found" / Config errors

**Problem:** Configuration fails to load.

**Solutions:**
1. Ensure `~/.m31a/config.toml` exists and is valid TOML:
   ```bash
   m31a --check-config  # (not yet implemented — manually validate)
   ```
2. Check for unknown keys in config (M31 Autonomous warns about typos)
3. Verify all `${VAR}` references have corresponding env vars set
4. Ensure boolean fields use `true`/`false` (not `yes`/`no`)
5. Config path precedence: `M31A_CONFIG` env → `~/.m31a/config.toml` → defaults

---

## Permission Prompts Time Out

**Problem:** Permission modal closes before you respond.

**Solutions:**
1. Increase timeout:
   ```toml
   [permissions]
   timeout_seconds = 600  # 10 minutes
   ```
2. Set per-agent default actions to reduce prompts
3. Add specific rules to auto-approve safe operations

---

## Slow Response / High Latency

**Solutions:**
1. Check health status in header (live/slow/down indicators)
2. Adjust health check thresholds:
   ```toml
   [features]
   healthcheck_live_ms = 5000   # More lenient "live" threshold
   healthcheck_slow_ms = 10000  # More lenient "slow" threshold
   ```
3. Switch to a faster model with `/model`
4. Enable arbitrage to auto-select cheaper/faster models:
   ```toml
   [model]
   auto_arbitrage = true
   ```
5. Disable streaming if not needed (set `stream = false` in provider config)

---

## Streaming Stutters or Freezes

**Solutions:**
1. Reduce thinking block display lines:
   ```toml
   [ui]
   thinking_max_lines = 10
   ```
2. Compact mode reduces rendering overhead:
   ```toml
   [ui]
   compact_mode = true
   ```
3. Disable animations:
   ```toml
   [ui]
   animation_speed = "none"
   ```

---

## Keychain Unavailable

**Problem:** OS keychain integration fails.

**Solutions:**
- **Linux:** Install `secret-service` (GNOME/KDE) or use `pass`:
  ```bash
  sudo apt install gnome-keyring libsecret-tools  # Debian/Ubuntu
  ```
- **macOS:** Keychain should work automatically via `/usr/bin/security`
- **Windows:** Credential Manager should be available by default
- **Fallback:** API keys can be stored in config file or env vars instead

---

## Session Not Found

**Problem:** Cannot load or switch to a session.

**Solutions:**
1. List available sessions: `/session list`
2. Sessions are stored in `~/.m31a/sessions/` — check directory exists
3. Session retention config:
   ```toml
   [features]
   session_retention_days = 60  # Keep sessions longer
   ```

---

## Provider Rate Limited (429)

**Solutions:**
1. M31 Autonomous handles backoff automatically (configurable):
   ```toml
   [features]
   rate_limit_backoff_secs = 120  # Wait 2 minutes before retry
   ```
2. Enable auto-fallback to alternate provider:
   ```toml
   [provider]
   auto_fallback = true
   ```
3. Switch models with `/model`

---

## Logs

M31 Autonomous logs to stderr. Set verbosity:

```bash
export M31A_LOG_LEVEL=debug   # Most verbose
export M31A_LOG_LEVEL=info    # Default
export M31A_LOG_LEVEL=warn    # Only warnings and errors
export M31A_LOG_FORMAT=json   # Structured JSON logs
```

---

## Debugging Tips

1. Check version: `m31a --version`
2. Inspect config at runtime: `/config` inside M31 Autonomous
3. Check provider health endpoint is reachable:
   ```bash
   curl -I https://openrouter.ai/api/v1/models
   ```
4. Verify TOML syntax:
   ```bash
   python3 -c "import tomllib; tomllib.load(open('~/.m31a/config.toml', 'rb'))"
   ```
