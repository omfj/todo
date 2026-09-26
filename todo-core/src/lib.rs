pub mod db;
pub mod models;

pub use db::Database;
pub use models::{Task, Workspace, WorkspaceStats};

pub const STATE_LAST_WORKSPACE_ID: &str = "last_workspace_id";
