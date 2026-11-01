# `m31a-web-search`

Real-time web search retrieval client and documentation lookup extension for **M31A**.

---

## Overview

`m31a-web-search` allows agents to query live web search engines to retrieve up-to-date documentation, API references, library release notes, and error solutions that may not be present in static training weights.

---

## User-Facing Capabilities

- **Live Documentation Search (`--search` flag or `tool_search`)**: Enables agents to search the web for latest library versions, syntax changes, and troubleshooting threads.
- **Engine Support**: Integrates with major search APIs (Google, Bing, Brave Search, DuckDuckGo, SerpAPI).

---

## Developer Guide & Architecture

### Key Types

- `SearchRequest`: Query string, target engine, max result count, language, and country filters.
- `SearchResponse`: List of `SearchResult` items containing title, snippet, URL, and relevance score.
- `SearchProvider` trait: Extensible interface for search engine integrations.

---

## Testing

```bash
cargo test -p m31a-web-search
```
