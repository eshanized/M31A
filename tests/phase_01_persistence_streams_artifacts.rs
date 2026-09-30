//! Phase 01: Hybrid Persistence Integration Tests
//!
//! Verifies FileStream append-only storage, FsArtifactStore blob persistence,
//! path sanitization, and SQLite pool & migration initialization.

use m31a::error::M31AError;
use m31a::ids::ArtifactId;
use m31a::persistence::{
    ArtifactStore, FileStream, FsArtifactStore, StreamStore, create_pool, initialize_database,
};
use tempfile::tempdir;

#[tokio::test]
async fn test_file_stream_sequential_appends_and_reads() {
    // Arrange: create tempdir and file stream
    let dir = tempdir().unwrap();
    let stream_store = FileStream::new(dir.path());
    let stream_name = "test_audit_stream";

    // Act: write 100 sequential events
    for i in 0..100 {
        let entry = format!("audit-event-{:04}", i);
        stream_store
            .append(stream_name, entry.as_bytes())
            .await
            .unwrap();
    }

    // Assert: read all entries and verify sequential order
    let entries = stream_store.read_all(stream_name).await.unwrap();
    assert_eq!(entries.len(), 100);
    for (i, entry) in entries.iter().enumerate() {
        let expected = format!("audit-event-{:04}", i);
        assert_eq!(entry, expected.as_bytes());
    }
}

#[tokio::test]
async fn test_file_stream_binary_data_with_null_bytes() {
    // Arrange: create stream with raw binary content including null bytes
    let dir = tempdir().unwrap();
    let stream_store = FileStream::new(dir.path());
    let stream_name = "binary_stream";
    let binary_payload = vec![
        0x00, 0x01, 0x02, 0x00, 0xFF, 0xFE, 0xFD, 0x00, 0x42, 0xAA, 0x55,
    ];

    // Act: append binary data
    stream_store
        .append(stream_name, &binary_payload)
        .await
        .unwrap();

    // Assert: read back and verify byte-exact match
    let entries = stream_store.read_all(stream_name).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0], binary_payload);
}

#[tokio::test]
async fn test_file_stream_path_sanitization() {
    // Arrange: stream names attempting directory traversal
    let dir = tempdir().unwrap();
    let stream_store = FileStream::new(dir.path());

    // Act: append to stream with ../ and nested paths
    stream_store
        .append("../../../escape_stream", b"sandboxed data")
        .await
        .unwrap();
    stream_store
        .append("nested/sub/stream", b"nested data")
        .await
        .unwrap();

    // Assert: stream files stay within dir.path()
    let read_escape = stream_store
        .read_all("../../../escape_stream")
        .await
        .unwrap();
    assert_eq!(read_escape.len(), 1);
    assert_eq!(read_escape[0], b"sandboxed data");

    let read_nested = stream_store.read_all("nested/sub/stream").await.unwrap();
    assert_eq!(read_nested.len(), 1);
    assert_eq!(read_nested[0], b"nested data");
}

#[tokio::test]
async fn test_file_stream_nonexistent_stream_empty() {
    // Arrange: store with no streams created
    let dir = tempdir().unwrap();
    let stream_store = FileStream::new(dir.path());

    // Act: read nonexistent stream
    let entries = stream_store.read_all("missing_stream").await.unwrap();

    // Assert: returns empty list without error
    assert!(entries.is_empty());
}

#[tokio::test]
async fn test_fs_artifact_store_store_and_retrieve_roundtrip() {
    // Arrange: create artifact store and payload
    let dir = tempdir().unwrap();
    let store = FsArtifactStore::new(dir.path());
    let artifact_id = ArtifactId::new();
    let payload = b"critical execution report or diff output";

    // Act: store artifact
    let stored_path = store
        .store(artifact_id, payload, "txt")
        .await
        .expect("Store should succeed");

    // Assert: path exists and contains artifact ID
    assert!(stored_path.exists());
    assert!(
        stored_path
            .to_string_lossy()
            .contains(&artifact_id.to_string())
    );

    // Act: retrieve artifact
    let retrieved = store
        .retrieve(artifact_id, "txt")
        .await
        .expect("Retrieve should succeed");

    // Assert: contents match
    assert_eq!(retrieved, payload);
}

#[tokio::test]
async fn test_fs_artifact_store_path_isolation() {
    // Arrange: malicious extension with traversal characters
    let dir = tempdir().unwrap();
    let store = FsArtifactStore::new(dir.path());
    let artifact_id = ArtifactId::new();
    let payload = b"secure blob";

    // Act: store with injection attempt in extension
    let stored_path = store
        .store(artifact_id, payload, "../../etc/passwd;rm -rf")
        .await
        .unwrap();

    // Assert: path remains strictly inside artifact dir and contains only alphanumeric ext
    assert!(stored_path.starts_with(dir.path()));
    assert!(stored_path.to_string_lossy().ends_with(".etcpasswdrmrf"));
}

#[tokio::test]
async fn test_fs_artifact_store_overwrite_semantics() {
    // Arrange: existing artifact
    let dir = tempdir().unwrap();
    let store = FsArtifactStore::new(dir.path());
    let artifact_id = ArtifactId::new();

    store
        .store(artifact_id, b"version 1 payload", "bin")
        .await
        .unwrap();

    // Act: overwrite with version 2
    store
        .store(artifact_id, b"version 2 updated payload", "bin")
        .await
        .unwrap();

    // Assert: retrieve returns updated payload
    let retrieved = store.retrieve(artifact_id, "bin").await.unwrap();
    assert_eq!(retrieved, b"version 2 updated payload");
}

#[tokio::test]
async fn test_fs_artifact_store_nonexistent_error() {
    // Arrange: nonexistent artifact ID
    let dir = tempdir().unwrap();
    let store = FsArtifactStore::new(dir.path());
    let missing_id = ArtifactId::new();

    // Act: retrieve nonexistent artifact
    let result = store.retrieve(missing_id, "log").await;

    // Assert: returns PersistenceError
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        M31AError::PersistenceError(_)
    ));
}

#[tokio::test]
async fn test_sqlite_pool_creation_and_wal_mode() {
    // Arrange: db path in tempdir
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_pool.db");

    // Act: create pool
    let pool = create_pool(&db_path).await.unwrap();

    // Assert: PRAGMA journal_mode is WAL
    let journal_mode: (String,) = sqlx::query_as("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(journal_mode.0.to_lowercase(), "wal");

    // Assert: busy_timeout is set
    let busy_timeout: (i64,) = sqlx::query_as("PRAGMA busy_timeout")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(busy_timeout.0 > 0);
}

#[tokio::test]
async fn test_sqlite_schema_migrations_and_tables() {
    // Arrange: db path in tempdir
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_migrated.db");

    // Act: initialize database (creates pool + runs migrations)
    let pool = initialize_database(&db_path).await.unwrap();

    // Assert: all required tables exist
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let table_names: Vec<String> = tables.into_iter().map(|(name,)| name).collect();
    let required_tables = ["missions", "tasks", "agents", "event_log", "sessions"];

    for table in required_tables {
        assert!(
            table_names.contains(&table.to_string()),
            "Required table '{}' was not created by migrations. Existing tables: {:?}",
            table,
            table_names
        );
    }
}
