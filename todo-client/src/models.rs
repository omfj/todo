#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedField {
    pub ciphertext: String,
    pub nonce: String,
    pub version: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedWorkspace {
    pub id: i64,
    pub name: EncryptedField,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStats {
    pub workspace_id: i64,
    pub completed: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub completed: bool,
    pub archived: bool,
    pub due_date: Option<String>,
    pub workspace_id: i64,
    pub parent_task_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedTask {
    pub id: i64,
    pub title: EncryptedField,
    pub description: Option<EncryptedField>,
    pub completed: bool,
    pub archived: bool,
    pub due_date: Option<EncryptedField>,
    pub workspace_id: i64,
    pub parent_task_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
