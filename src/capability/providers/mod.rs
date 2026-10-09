//! Native local capability providers (CTL-02, D-01, D-03).

pub mod cli_git;
pub mod event_bus;
pub mod fs_artifacts;
pub mod local_fs;
pub mod local_lsp;
pub mod local_memory;
pub mod local_network;
pub mod local_process;
pub mod local_sandbox;
pub mod local_terminal;
pub mod local_verification;
pub mod local_web;
pub mod model_caller;
pub mod repo_graph;

pub use cli_git::{CliGitProvider, DisabledGitProvider};
pub use event_bus::EventBusProvider;
pub use fs_artifacts::FsArtifactStoreProvider;
pub use local_fs::LocalFileSystemProvider;
pub use local_lsp::{LocalLspProcessClient, LspSessionState, PersistentLspSession};
pub use local_memory::LocalMemoryProvider;
pub use local_network::LocalNetworkProvider;
pub use local_process::{LocalJobProvider, LocalProcessProvider};
pub use local_sandbox::LocalSandboxProvider;
pub use local_terminal::LocalTerminalProvider;
pub use local_verification::LocalVerificationProvider;
pub use local_web::LocalWebProvider;
pub use model_caller::ModelCallerProvider;
pub use repo_graph::RepositoryGraphProvider;
