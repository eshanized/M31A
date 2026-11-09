# `m31a-web-search`

> Real-time web search client, search engine abstractions (DuckDuckGo, Brave, Google, Bing), and snippet extraction for M31A.

---

## Why Does This Crate Exist?

Modern software development relies heavily on up-to-date documentation, API release notes, and community issue trackers. An autonomous agent whose training data cuts off cannot know about the newest breaking changes in Rust 2024 edition or a newly released crate version without searching the live web.

`m31a-web-search` equips M31A with real-time web research capabilities:
- Provides a clean trait abstraction across search providers (DuckDuckGo, Brave Search, Google, Bing, SerpAPI).
- Formats search results into structured, token-efficient summaries with citations and URLs.
- Handles query timeouts, rate limiting, and result deduplication.
- Supports site filtering (`site:docs.rs`) and time-range constraints.

---

## Core Concepts & How It Works

### 1. Pluggable Search Providers (`SearchEngine`)
Supports multiple search backends without code modifications:
- `DuckDuckGo`: Privacy-preserving, zero-API-key default.
- `Brave`: High-speed independent web index.
- `Google` & `Bing`: Traditional web indexes via official or custom APIs.
- `SerpApi`: Structured search engine results.

### 2. Snippet & Link Normalization
Search results are sanitized, HTML tags stripped, and structured into:
- Title, URL, and snippet text.
- Metadata (cached timestamp, engine used).
- Result ranking scores.

---

## Key Types & Structs

| Struct / Enum | Description |
| :--- | :--- |
| `SearchRequest` | Query parameters (text, engine, max results, site filter, time range). |
| `SearchResponse` | Vector of ranked `SearchResult` items with query execution duration. |
| `SearchResult` | Normalized title, destination URL, snippet snippet text, and position. |
| `SearchEngine` | Supported search engine enumeration (`DuckDuckGo`, `Brave`, `Google`, etc.). |
| `WebSearchService` | Client manager orchestrating HTTP requests and fallback engines. |

---

## Practical Usage Example

```rust
use m31a_web_search::{SafeSearchLevel, SearchEngine, SearchRequest, WebSearchService};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service = WebSearchService::new();

    let request = SearchRequest {
        query: "ratatui 0.29 breaking changes crossterm".into(),
        engine: SearchEngine::DuckDuckGo,
        max_results: 3,
        offset: 0,
        language: Some("en".into()),
        region: None,
        safe_search: SafeSearchLevel::Moderate,
        time_range: None,
        site_filter: None,
        file_type: None,
    };

    let response = service.search(request).await?;
    println!("Found {} results in {} ms:", response.results.len(), response.duration_ms);

    for result in response.results {
        println!("- [{}]({}): {}", result.title, result.url, result.snippet);
    }

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-web-search
```
