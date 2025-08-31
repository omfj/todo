#![allow(dead_code)]

use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub completed: bool,
    pub workspace_id: i64,
    pub parent_task_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
