//! Dedicated integration tests for `m31a-web-search` (src/lib.rs).
//!
//! Validates:
//! - SSRF defense and public URL validation blocking loopback, RFC1918, link-local metadata, and credentials
//! - SearchProvider trait implementation with real query execution
//! - SearchManager multi-engine registration, query routing, and TTL caching
//! - Error boundaries for unavailable search engines

use m31a_web_search::{
    validate_public_url, SafeSearchLevel, SearchEngine, SearchFuture, SearchManager,
    SearchProvider, SearchRequest, SearchResponse, SearchResult, SearchResultMetadata,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct MockSearchProvider {
    engine: SearchEngine,
    name: &'static str,
    call_count: Arc<AtomicUsize>,
}

impl SearchProvider for MockSearchProvider {
    fn engine(&self) -> SearchEngine {
        self.engine
    }

    fn name(&self) -> &str {
        self.name
    }

    fn supports_pagination(&self) -> bool {
        true
    }

    fn max_results_per_page(&self) -> usize {
        10
    }

    fn search(&self, request: SearchRequest) -> SearchFuture<'_> {
        let count = Arc::clone(&self.call_count);
        Box::pin(async move {
            count.fetch_add(1, Ordering::SeqCst);
            let results = vec![
                SearchResult {
                    title: format!("Result 1 for {}", request.query),
                    url: "https://example.com/item1".to_string(),
                    snippet: "Comprehensive guide to systems programming".to_string(),
                    source: "mock".to_string(),
                    position: 1,
                    metadata: Some(SearchResultMetadata {
                        published_date: Some("2026-11-01".to_string()),
                        author: Some("Dev Lead".to_string()),
                        domain: "example.com".to_string(),
                        is_sponsored: false,
                        rich_snippet: None,
                    }),
                },
                SearchResult {
                    title: format!("Result 2 for {}", request.query),
                    url: "https://example.org/item2".to_string(),
                    snippet: "Architecture and design notes".to_string(),
                    source: "mock".to_string(),
                    position: 2,
                    metadata: None,
                },
            ];

            Ok(SearchResponse {
                query: request.query,
                engine: self.engine,
                results,
                total_results: Some(42),
                search_time_ms: 12,
                timestamp: 1700000000,
            })
        })
    }

    fn get_rate_limit(&self) -> Option<u32> {
        Some(100)
    }
}

#[tokio::test]
async fn test_ssrf_adversarial_rejection() {
    // Loopback IPv4
    assert!(validate_public_url("http://127.0.0.1:8080/admin").await.is_err());
    assert!(validate_public_url("http://localhost:9090").await.is_err());

    // AWS/GCP/Azure link-local metadata IP
    assert!(validate_public_url("http://169.254.169.254/latest/meta-data").await.is_err());

    // Private RFC1918 subnets
    assert!(validate_public_url("http://10.0.0.1/internal").await.is_err());
    assert!(validate_public_url("http://172.16.10.20/db").await.is_err());
    assert!(validate_public_url("http://192.168.1.254/router").await.is_err());

    // Loopback IPv6
    assert!(validate_public_url("http://[::1]:8080/").await.is_err());

    // Dangerous protocols
    assert!(validate_public_url("file:///etc/passwd").await.is_err());
    assert!(validate_public_url("ftp://ftp.example.com").await.is_err());
    assert!(validate_public_url("gopher://gopher.example.com").await.is_err());

    // Credential stuffing in URL
    assert!(validate_public_url("https://root:secret@api.example.com").await.is_err());

    // Internal top-level domain
    assert!(validate_public_url("http://backend.internal/api").await.is_err());
    assert!(validate_public_url("http://node.local/status").await.is_err());

    // Legitimate public HTTPS URLs must be allowed
    assert!(validate_public_url("https://crates.io").await.is_ok());
    assert!(validate_public_url("https://example.com/search").await.is_ok());
}

#[tokio::test]
async fn test_search_manager_routing_and_ttl_caching() {
    let mut manager = SearchManager::new(SearchEngine::Brave);

    let call_count = Arc::new(AtomicUsize::new(0));
    let provider = MockSearchProvider {
        engine: SearchEngine::Brave,
        name: "mock-brave",
        call_count: Arc::clone(&call_count),
    };

    manager.register(SearchEngine::Brave, Box::new(provider));

    let request = SearchRequest {
        query: "rust async concurrency".to_string(),
        engine: SearchEngine::Brave,
        max_results: 5,
        offset: 0,
        language: Some("en".to_string()),
        region: None,
        safe_search: SafeSearchLevel::Moderate,
        time_range: None,
        site_filter: None,
        file_type: None,
    };

    // First search: executes provider
    let resp1 = manager.search(request.clone()).await.unwrap();
    assert_eq!(resp1.results.len(), 2);
    assert_eq!(resp1.results[0].title, "Result 1 for rust async concurrency");
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Second search with identical request: served from in-memory TTL cache
    let resp2 = manager.search(request).await.unwrap();
    assert_eq!(resp2.results.len(), 2);
    assert_eq!(resp2.query, resp1.query);
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "Second call must hit cache without invoking provider"
    );
}

#[tokio::test]
async fn test_search_all_and_unregistered_engine() {
    let mut manager = SearchManager::new(SearchEngine::Google);

    let count_ddg = Arc::new(AtomicUsize::new(0));
    manager.register(
        SearchEngine::DuckDuckGo,
        Box::new(MockSearchProvider {
            engine: SearchEngine::DuckDuckGo,
            name: "mock-ddg",
            call_count: count_ddg,
        }),
    );

    // search_all should invoke all registered engines
    let all_res = manager.search_all("distributed consensus".to_string(), 5).await.unwrap();
    assert_eq!(all_res.len(), 1);
    assert_eq!(all_res[0].engine, SearchEngine::DuckDuckGo);

    // Searching with unregistered engine (Google) should fail
    let req = SearchRequest {
        query: "unsupported query".to_string(),
        engine: SearchEngine::Google,
        max_results: 5,
        offset: 0,
        language: None,
        region: None,
        safe_search: SafeSearchLevel::Moderate,
        time_range: None,
        site_filter: None,
        file_type: None,
    };

    let res = manager.search(req).await;
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("Search engine Google not available"));
}
