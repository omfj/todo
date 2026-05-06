use std::{env, path::Path};

use chrono::{DateTime, Utc};
use sqlx::{FromRow, sqlite::SqlitePool};
use todo_client::{Task, Workspace, WorkspaceStats};

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

        Ok(Database { pool })
    }

    pub async fn run_migrations(
        &self,
        migrator: &'static sqlx::migrate::Migrator,
    ) -> anyhow::Result<()> {
        migrator.run(&self.pool).await?;

        Ok(())
    }

    pub async fn get_workspaces(&self) -> anyhow::Result<Vec<Workspace>> {
        let rows = sqlx::query_as::<_, WorkspaceRow>(
            "SELECT id, name, created_at, updated_at FROM workspaces ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_workspace_stats(&self) -> anyhow::Result<Vec<WorkspaceStats>> {
        let rows = sqlx::query_as::<_, WorkspaceStatsRow>(
            "SELECT w.id AS workspace_id,
                    COALESCE(SUM(CASE WHEN t.completed = 1 THEN 1 ELSE 0 END), 0) AS completed,
                    COUNT(t.id) AS total
             FROM workspaces w
             LEFT JOIN tasks t ON t.workspace_id = w.id AND t.archived = 0
             GROUP BY w.id",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_tasks_for_workspace(&self, workspace_id: i64) -> anyhow::Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT id, title, description, completed, archived, due_date, workspace_id, parent_task_id, created_at, updated_at
             FROM tasks WHERE workspace_id = ? AND archived = 0 ORDER BY created_at",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn create_workspace(&self, name: &str) -> anyhow::Result<i64> {
        let result = sqlx::query("INSERT INTO workspaces (name) VALUES (?)")
            .bind(name)
            .execute(&self.pool)
            .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn create_task(&self, title: &str, workspace_id: i64) -> anyhow::Result<i64> {
        let result = sqlx::query("INSERT INTO tasks (title, workspace_id) VALUES (?, ?)")
            .bind(title)
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn create_subtask(
        &self,
        title: &str,
        workspace_id: i64,
        parent_task_id: i64,
    ) -> anyhow::Result<i64> {
        let result =
            sqlx::query("INSERT INTO tasks (title, workspace_id, parent_task_id) VALUES (?, ?, ?)")
                .bind(title)
                .bind(workspace_id)
                .bind(parent_task_id)
                .execute(&self.pool)
                .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn toggle_task_completion(&self, task_id: i64) -> anyhow::Result<()> {
        sqlx::query("UPDATE tasks SET completed = NOT completed WHERE id = ?")
            .bind(task_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn archive_completed_tasks(&self, workspace_id: i64) -> anyhow::Result<()> {
        sqlx::query("UPDATE tasks SET archived = 1 WHERE workspace_id = ? AND completed = 1")
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn update_workspace_name(&self, workspace_id: i64, name: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE workspaces SET name = ? WHERE id = ?")
            .bind(name)
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn update_task_name(&self, task_id: i64, title: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE tasks SET title = ? WHERE id = ?")
            .bind(title)
            .bind(task_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn update_task_due_date(
        &self,
        task_id: i64,
        due_date: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE tasks SET due_date = ? WHERE id = ?")
            .bind(due_date)
            .bind(task_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn delete_workspace(&self, workspace_id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM tasks WHERE workspace_id = ?")
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;

        sqlx::query("DELETE FROM workspaces WHERE id = ?")
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn delete_task(&self, task_id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM tasks WHERE id = ?")
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

impl From<WorkspaceRow> for Workspace {
    fn from(row: WorkspaceRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
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

impl From<TaskRow> for Task {
    fn from(row: TaskRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            description: row.description,
            completed: row.completed,
            archived: row.archived,
            due_date: row.due_date,
            workspace_id: row.workspace_id,
            parent_task_id: row.parent_task_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
