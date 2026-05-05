use crate::models::{Task, Workspace, WorkspaceStats};
use sqlx::sqlite::SqlitePool;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn connect() -> anyhow::Result<Self> {
        let config_dir = dirs::state_dir()
            .or_else(dirs::config_dir)
            .or_else(|| dirs::home_dir().map(|h| h.join(".local/state")))
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;

        let db_path = config_dir.join("todo").join("data");
        std::fs::create_dir_all(&db_path)?;

        let db_file = db_path.join("todo.db");
        let database_url = format!("sqlite:{}?mode=rwc", db_file.display());

        let pool = SqlitePool::connect(&database_url).await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Database { pool })
    }

    pub async fn get_workspaces(&self) -> anyhow::Result<Vec<Workspace>> {
        let rows = sqlx::query_as::<_, Workspace>(
            "SELECT id, name, created_at, updated_at FROM workspaces ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_workspace_stats(&self) -> anyhow::Result<Vec<WorkspaceStats>> {
        let rows = sqlx::query_as::<_, WorkspaceStats>(
            "SELECT w.id AS workspace_id,
                    COALESCE(SUM(CASE WHEN t.completed = 1 THEN 1 ELSE 0 END), 0) AS completed,
                    COUNT(t.id) AS total
             FROM workspaces w
             LEFT JOIN tasks t ON t.workspace_id = w.id AND t.archived = 0
             GROUP BY w.id",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_tasks_for_workspace(&self, workspace_id: i64) -> anyhow::Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, Task>(
            "SELECT id, title, description, completed, archived, due_date, workspace_id, parent_task_id, created_at, updated_at
             FROM tasks WHERE workspace_id = ? AND archived = 0 ORDER BY created_at",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn create_workspace(&self, name: &str) -> anyhow::Result<i64> {
        let result = sqlx::query!("INSERT INTO workspaces (name) VALUES (?)", name)
            .execute(&self.pool)
            .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn create_task(&self, title: &str, workspace_id: i64) -> anyhow::Result<i64> {
        let result = sqlx::query!(
            "INSERT INTO tasks (title, workspace_id) VALUES (?, ?)",
            title,
            workspace_id
        )
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
        sqlx::query!(
            "UPDATE tasks SET completed = NOT completed WHERE id = ?",
            task_id
        )
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
        sqlx::query!(
            "UPDATE workspaces SET name = ? WHERE id = ?",
            name,
            workspace_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_task_name(&self, task_id: i64, title: &str) -> anyhow::Result<()> {
        sqlx::query!("UPDATE tasks SET title = ? WHERE id = ?", title, task_id)
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
        sqlx::query!("DELETE FROM tasks WHERE workspace_id = ?", workspace_id)
            .execute(&self.pool)
            .await?;

        sqlx::query!("DELETE FROM workspaces WHERE id = ?", workspace_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn delete_task(&self, task_id: i64) -> anyhow::Result<()> {
        sqlx::query!("DELETE FROM tasks WHERE id = ?", task_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
