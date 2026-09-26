use std::{env, path::Path};

use chrono::{DateTime, Utc};
use sqlx::{FromRow, sqlite::SqlitePool};
use todo_client::{EncryptedField, EncryptedTask, EncryptedWorkspace, WorkspaceStats};

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn connect() -> anyhow::Result<Self> {
        let database_url = env::var("DATABASE_URL")?;
        if let Some(db_path) = sqlite_database_path(&database_url)
            && let Some(parent) = Path::new(db_path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        let pool = SqlitePool::connect(&database_url).await?;
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&pool)
            .await?;

        Ok(Database { pool })
    }

    pub async fn run_migrations(
        &self,
        migrator: &'static sqlx::migrate::Migrator,
    ) -> anyhow::Result<()> {
        migrator.run(&self.pool).await?;

        Ok(())
    }

    pub async fn ensure_user(&self, user_id: &str) -> anyhow::Result<()> {
        sqlx::query("INSERT OR IGNORE INTO users (id) VALUES (?)")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_workspaces(&self, user_id: &str) -> anyhow::Result<Vec<EncryptedWorkspace>> {
        let rows = sqlx::query_as::<_, WorkspaceRow>(
            "SELECT id, name, created_at, updated_at FROM workspaces WHERE user_id = ? ORDER BY created_at",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(WorkspaceRow::try_into).collect()
    }

    pub async fn get_workspace_stats(&self, user_id: &str) -> anyhow::Result<Vec<WorkspaceStats>> {
        let rows = sqlx::query_as::<_, WorkspaceStatsRow>(
            "SELECT w.id AS workspace_id,
                    COALESCE(SUM(CASE WHEN t.completed = 1 THEN 1 ELSE 0 END), 0) AS completed,
                    COUNT(t.id) AS total
             FROM workspaces w
             LEFT JOIN tasks t ON t.user_id = w.user_id AND t.workspace_id = w.id AND t.archived = 0
             WHERE w.user_id = ?
             GROUP BY w.id",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_tasks_for_workspace(
        &self,
        user_id: &str,
        workspace_id: i64,
    ) -> anyhow::Result<Vec<EncryptedTask>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT id, title, description, completed, archived, due_date, workspace_id, parent_task_id, created_at, updated_at
             FROM tasks WHERE user_id = ? AND workspace_id = ? AND archived = 0 ORDER BY created_at",
        )
        .bind(user_id)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(TaskRow::try_into).collect()
    }

    pub async fn create_workspace(
        &self,
        user_id: &str,
        name: &EncryptedField,
    ) -> anyhow::Result<i64> {
        let result = sqlx::query("INSERT INTO workspaces (user_id, name) VALUES (?, ?)")
            .bind(user_id)
            .bind(encrypted_field_to_string(name)?)
            .execute(&self.pool)
            .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn create_task(
        &self,
        user_id: &str,
        title: &EncryptedField,
        workspace_id: i64,
    ) -> anyhow::Result<i64> {
        let result =
            sqlx::query("INSERT INTO tasks (user_id, title, workspace_id) VALUES (?, ?, ?)")
                .bind(user_id)
                .bind(encrypted_field_to_string(title)?)
                .bind(workspace_id)
                .execute(&self.pool)
                .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn create_subtask(
        &self,
        user_id: &str,
        title: &EncryptedField,
        workspace_id: i64,
        parent_task_id: i64,
    ) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT INTO tasks (user_id, title, workspace_id, parent_task_id) VALUES (?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(encrypted_field_to_string(title)?)
        .bind(workspace_id)
        .bind(parent_task_id)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn toggle_task_completion(&self, user_id: &str, task_id: i64) -> anyhow::Result<()> {
        sqlx::query("UPDATE tasks SET completed = NOT completed WHERE user_id = ? AND id = ?")
            .bind(user_id)
            .bind(task_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn archive_completed_tasks(
        &self,
        user_id: &str,
        workspace_id: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE tasks SET archived = 1 WHERE user_id = ? AND workspace_id = ? AND completed = 1",
        )
            .bind(user_id)
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn update_workspace_name(
        &self,
        user_id: &str,
        workspace_id: i64,
        name: &EncryptedField,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE workspaces SET name = ? WHERE user_id = ? AND id = ?")
            .bind(encrypted_field_to_string(name)?)
            .bind(user_id)
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn update_task_name(
        &self,
        user_id: &str,
        task_id: i64,
        title: &EncryptedField,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE tasks SET title = ? WHERE user_id = ? AND id = ?")
            .bind(encrypted_field_to_string(title)?)
            .bind(user_id)
            .bind(task_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn update_task_due_date(
        &self,
        user_id: &str,
        task_id: i64,
        due_date: Option<&EncryptedField>,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE tasks SET due_date = ? WHERE user_id = ? AND id = ?")
            .bind(due_date.map(encrypted_field_to_string).transpose()?)
            .bind(user_id)
            .bind(task_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn delete_workspace(&self, user_id: &str, workspace_id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM tasks WHERE user_id = ? AND workspace_id = ?")
            .bind(user_id)
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;

        sqlx::query("DELETE FROM workspaces WHERE user_id = ? AND id = ?")
            .bind(user_id)
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn delete_task(&self, user_id: &str, task_id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM tasks WHERE user_id = ? AND id = ?")
            .bind(user_id)
            .bind(task_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

fn sqlite_database_path(database_url: &str) -> Option<&str> {
    let path = database_url
        .strip_prefix("sqlite://")
        .or_else(|| database_url.strip_prefix("sqlite:"))?;
    let path = path.split_once('?').map_or(path, |(path, _)| path);

    if path.is_empty() || path == ":memory:" {
        None
    } else {
        Some(path)
    }
}

#[derive(FromRow)]
struct WorkspaceRow {
    id: i64,
    name: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<WorkspaceRow> for EncryptedWorkspace {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: row.id,
            name: encrypted_field_from_string(&row.name)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[derive(FromRow)]
struct WorkspaceStatsRow {
    workspace_id: i64,
    completed: i64,
    total: i64,
}

impl From<WorkspaceStatsRow> for WorkspaceStats {
    fn from(row: WorkspaceStatsRow) -> Self {
        Self {
            workspace_id: row.workspace_id,
            completed: row.completed,
            total: row.total,
        }
    }
}

#[derive(FromRow)]
struct TaskRow {
    id: i64,
    title: String,
    description: Option<String>,
    completed: bool,
    archived: bool,
    due_date: Option<String>,
    workspace_id: i64,
    parent_task_id: Option<i64>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<TaskRow> for EncryptedTask {
    type Error = anyhow::Error;

    fn try_from(row: TaskRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: row.id,
            title: encrypted_field_from_string(&row.title)?,
            description: row
                .description
                .as_deref()
                .map(encrypted_field_from_string)
                .transpose()?,
            completed: row.completed,
            archived: row.archived,
            due_date: row
                .due_date
                .as_deref()
                .map(encrypted_field_from_string)
                .transpose()?,
            workspace_id: row.workspace_id,
            parent_task_id: row.parent_task_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

fn encrypted_field_to_string(field: &EncryptedField) -> anyhow::Result<String> {
    serde_json::to_string(field).map_err(Into::into)
}

fn encrypted_field_from_string(value: &str) -> anyhow::Result<EncryptedField> {
    serde_json::from_str(value).map_err(Into::into)
}
