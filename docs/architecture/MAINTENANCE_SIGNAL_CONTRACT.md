# Maintenance Signal Contract Specification

## 1. Domain Types & Enums

### Signal Provider
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalProvider {
    GitHub,
    GitLab,
    LocalAnalysis,
    VibeLoop,
    ScheduledJob,
    ExternalWebhook,
}
```

### Maintenance Category
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaintenanceCategory {
    BrokenCi,
    DependencyUpdate,
    IssueBug,
    ApiMigration,
    DocDrift,
    SecurityAdvisory,
    ArchitectureDrift,
}
```

### Signal Severity
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SignalSeverity {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}
```

### Signal Trust Status
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalTrustStatus {
    Verified,
    Unverified,
    Internal,
}
```

---

## 2. Canonical MaintenanceSignal Data Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaintenanceSignal {
    pub id: String,
    pub repo_id: String,
    pub provider: SignalProvider,
    pub category: MaintenanceCategory,
    pub provider_event_id: Option<String>,
    pub external_url: Option<String>,
    pub event_timestamp_ms: i64,
    pub observed_revision: Option<String>,
    pub severity: SignalSeverity,
    pub summary: String,
    pub raw_payload: serde_json::Value,
    pub ingested_at_epoch_ms: i64,
    pub dedup_fingerprint: String,
    pub trust_status: SignalTrustStatus,
}
```

### Deduplication Fingerprint Calculation
The deduplication fingerprint guarantees idempotent ingestion across repeated webhook deliveries or scheduled polls. It is computed as:

$$\text{dedup\_fingerprint} = \text{SHA-256}\Big(\text{repo\_id} \parallel \text{provider} \parallel \text{category} \parallel \text{provider\_event\_id} \parallel \text{observed\_revision}\Big)$$

If `provider_event_id` is absent, the normalized summary string is used as a fallback anchor.

---

## 3. Webhook Intake Security Invariants

1. **HMAC-SHA256 Signature Verification**: Inbound GitHub webhooks must provide `X-Hub-Signature-256` matching the SHA-256 HMAC of the raw payload using the configured webhook secret. The comparison uses constant-time byte equality to prevent timing attacks.
2. **Payload Size Bound**: Webhook payloads exceeding 5 MB (`5,242,880` bytes) are immediately rejected with HTTP 413.
3. **Replay & Timestamp Protection**: Webhook timestamps differing from host UTC by more than 10 minutes are flagged as unverified or rejected.
4. **Untrusted Input Sanitization**: Strings extracted from issue bodies or commit messages are stripped of null bytes (`\0`), ANSI control escape characters (`\x1b`), and bounded in length before storage or prompt formatting.
