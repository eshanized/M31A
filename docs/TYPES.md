# Type Reference

Core types used across M31 Autonomous, defined in `internal/types/`.

---

## Chat & Messages

```go
type ChatRequest struct {
    Model       string            // Model ID
    Messages    []Message          // Conversation messages
    Stream      bool               // Enable streaming
    Temperature float64            // Generation temperature
    MaxTokens   int                // Max output tokens
    Tools       []ToolDef          // Available tools
    ToolChoice  string             // "auto", "required", "none"
    Stop        []string           // Stop sequences
}

type ChatResponse struct {
    ID         string
    Model      string
    Choices    []Choice            // Response choices
    Usage      Usage               // Token usage
    SystemFingerprint string
}

type Choice struct {
    Index   int
    Message Message
    FinishReason string             // "stop", "length", "tool_calls"
}

type Message struct {
    Role       string              // "system", "user", "assistant", "tool"
    Content    string              // Text content
    ToolCallID string              // Tool call identifier
    ToolCalls  []ToolCall          // Tool invocations
}

type StreamChunk struct {
    ID                string
    Model             string
    Delta             Message            // Incremental message
    FinishReason      string
    Usage             *Usage             // Final usage (last chunk)
}
```

---

## Models

```go
type ModelInfo struct {
    ID            string       // Model identifier (e.g., "openai/gpt-4o")
    Name          string       // Human-readable name
    Provider      ProviderName // "openrouter", "zen", or "nvidia"
    Description   string
    Pricing       Pricing      // Cost per token
    ContextLength int          // Max context window
    Capabilities  CapFlags     // Feature flags
    Modified      time.Time    // Last update timestamp
}

type Pricing struct {
    PromptCost     float64 // Cost per 1K input tokens
    CompletionCost float64 // Cost per 1K output tokens
}

type CapFlags struct {
    Tools    bool // Function/tool calling support
    Reasoning bool // Reasoning/thinking model
    Vision   bool // Image/multimodal input
}
```

---

## Token Usage & Cost

```go
type Usage struct {
    PromptTokens     int
    CompletionTokens int
    TotalTokens      int
}

type UsageSample struct {
    Timestamp time.Time
    Usage     Usage
}

type UsageTracker struct {
    Samples   []UsageSample
    MaxAge    time.Duration   // Max age before samples expire
    EMAAlpha  float64          // Exponential moving average alpha
}
```

---

## Streaming

```go
type StreamIterator struct {
    Next func() (*StreamChunk, error) // Blocking read
}
```

---

## Tools

```go
type ToolDef struct {
    Name        string      // Tool name
    Description string      // Tool description
    InputSchema Schema      // JSON Schema for input
}

type ToolCall struct {
    ID      string
    Type    string          // "function"
    Function FunctionCall
}

type FunctionCall struct {
    Name      string
    Arguments string          // JSON-encoded arguments
}

type ToolResult struct {
    ToolCallID string
    Output    string
    Error     string
}
```

---

## Health Monitoring

```go
type HealthReport struct {
    Provider   string     // Provider name
    Status     string     // "live", "slow", "down"
    LatencyMs  int64      // Response time in ms
    Error      string
}

type HealthCheckType int
const (
    HealthPending HealthCheckType = iota
    HealthLive
    HealthSlow
    HealthDown
)
```

---

## Sessions

```go
type SessionID string    // Hex string (default 8 chars, configurable 4–16)

type Session struct {
    ID        SessionID
    Name      string
    Messages  []Message
    CreatedAt time.Time
    UpdatedAt time.Time
    Checkpoints []SessionID  // References to checkpoint snapshots
}
```

---

## Arbitrage

```go
type ComplexityLevel string
const (
	ComplexityTrivial  ComplexityLevel = "trivial"
	ComplexitySimple   ComplexityLevel = "simple"
	ComplexityModerate ComplexityLevel = "moderate"
	ComplexityComplex  ComplexityLevel = "complex"
)

type CostEstimate struct {
    ModelID      string
    Provider     string
    InputCost    float64
    OutputCost   float64
    TotalCost    float64
    Currency     string
    InputTokens  int
    OutputTokens int
}

type ArbitrageRecommendation struct {
    RecommendedModel CostEstimate
    Alternatives     []CostEstimate
    Complexity       ComplexityLevel
    Savings          float64
    Reason           string
}
```

---

## Verification

```go
type Attestation struct {
    BinaryHash string    // SHA-256 of binary
    Signature  string    // Cryptographic signature
    Timestamp  time.Time
}

type ValidationResult struct {
    Valid   bool
    Errors  []string
}
```

---

## Ghost Write

```go
type GhostFile struct {
    Path    string    // Target file path
    Content string    // Generated content
    Prompt  string    // Original prompt used
}

type GhostResult struct {
    Files    []GhostFile
    Warnings []string
}
```

---

## Config (internal)

```go
type RiskLevel string
const (
    RiskLevelLow          RiskLevel = "low"
    RiskLevelMedium       RiskLevel = "medium"
    RiskLevelHigh         RiskLevel = "high"
    RiskLevelDestructive  RiskLevel = "destructive"
)
```

---

## Constants

```go
// Default values
const (
    DefaultOpenRouterBaseURL = "https://openrouter.ai/api/v1"
    DefaultZenBaseURL        = "https://api.zen.com/v1"
    DefaultReferer           = "https://github.com/eshanized/M31A"
    DefaultAppTitle          = "M31 Autonomous"
    DefaultContextLength     = 128000
    DefaultHealthLiveMs      = 2000
    DefaultHealthSlowMs      = 5000
    SessionIDLength          = 8
    DefaultMaxRecentModels   = 10
    ModelCacheTTL            = 5 * time.Minute
    StaleCacheTTL            = 24 * time.Hour
    ConfigWatchInterval      = 5 * time.Second
    MaxProjectConfigDepth    = 3
)
```
