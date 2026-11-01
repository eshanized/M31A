//! `m31a-storage`: Relational persistence, append-only rollout logging, and transcript export.
//!
//! Manages SQLite session databases, thread histories, turn messages, binary attachments,
//! and provides exporters for Markdown, HTML, and JSONL transcripts.

pub mod attachment_store;
pub mod db;
pub mod history_recovery;
pub mod live_sync;
pub mod log_db;
pub mod migrations;
pub mod notes;
pub mod queue_store;
pub mod rollout;
pub mod state_file;
pub mod transcript_export;
pub mod worktree;

pub use state_file::{FieldPreservationPolicy, StateDocument, StateFileLock, StateTransaction};

pub use worktree::{
    DEFAULT_WORKTREE_KEEP_COUNT, ManagedWorktree, WorktreeManager, WorktreeSettings,
};

pub use attachment_store::{
    AttachmentMetadata, AttachmentStoreError, AttachmentStoreResult, FormatSpecificMetadata,
    ImageMetadata, LocalDiskAttachmentStore, ResolveFuture, ResolveRequest, StorageAttachmentStore,
    UploadFuture, UploadRequest, UploadResult, detect_mime_type, inspect_image_dimensions,
};
pub use db::{
    BoardPost, ChannelRecord, CheckpointRecord, ExecutionClaim, Goal, InboxItem, PathLockRecord,
    Storage, StoredEvidenceRecord, StoredSessionMemory, ThreadRecord, ThreadSectionRecord,
    ThreadSpawnEdgeRecord, ThreadStats, ThreadSummaryRecord, TurnRecord,
};
pub use history_recovery::{
    HistoryItemInfo, HistoryListItemsQuery, HistoryReadResult, HistoryRecoveryStore,
    HistorySearchQuery, HistorySearchResult, HistoryWindowInfo,
};
pub use live_sync::{LiveSyncManager, PendingThreadMetaUpdate};
pub use log_db::{AuditLogRecord, LogDatabase, LogLevel, LogQueryFilter};
pub use migrations::{AppliedMigrationRecord, BUILTIN_MIGRATIONS, Migration, MigrationEngine};
pub use notes::{
    MAX_NOTE_BYTES, NoteFileInfo, NoteSearchResult, NotesError, RolloutNotesManager,
    validate_and_normalize_path,
};
pub use queue_store::{QueueStore, QueueTaskRecord, TaskStatus};
pub use transcript_export::{TranscriptExporter, TranscriptItemRecord, TranscriptTurnGroup};
