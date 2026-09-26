pub mod client;
pub mod crypto;
pub mod models;

pub use client::Client;
pub use crypto::{CryptoKey, generate_recovery_phrase, normalize_phrase_for_storage};
pub use models::{
    EncryptedField, EncryptedTask, EncryptedWorkspace, Task, Workspace, WorkspaceStats,
};
