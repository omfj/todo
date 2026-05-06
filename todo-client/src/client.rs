use crate::{Task, Workspace, WorkspaceStats};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Client {
    endpoint_url: String,
    http: reqwest::Client,
}

impl Client {
    pub fn new(endpoint_url: String) -> Self {
        Self {
            endpoint_url: endpoint_url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    pub async fn get_workspaces(&self) -> anyhow::Result<Vec<Workspace>> {
        let response = self
            .http
            .get(self.url("/api/workspaces"))
            .send()
            .await?
            .error_for_status()?
            .json::<WorkspacesResponse>()
            .await?;

        Ok(response.workspaces)
    }

    pub async fn get_workspace_stats(&self) -> anyhow::Result<Vec<WorkspaceStats>> {
        let response = self
            .http
            .get(self.url("/api/workspaces"))
            .send()
            .await?
            .error_for_status()?
            .json::<WorkspacesResponse>()
            .await?;

        Ok(response.stats)
    }

    pub async fn get_tasks_for_workspace(&self, workspace_id: i64) -> anyhow::Result<Vec<Task>> {
        let tasks = self
            .http
            .get(self.url(&format!("/api/workspaces/{workspace_id}/tasks")))
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<Task>>()
            .await?;

        Ok(tasks)
    }

    pub async fn create_workspace(&self, name: &str) -> anyhow::Result<i64> {
        let response = self
            .http
            .post(self.url("/api/workspaces"))
            .json(&CreateWorkspaceRequest {
                name: name.to_string(),
            })
            .send()
            .await?
            .error_for_status()?
            .json::<IdResponse>()
            .await?;

        Ok(response.id)
    }

    pub async fn create_task(&self, title: &str, workspace_id: i64) -> anyhow::Result<i64> {
        self.create_task_request(title, workspace_id, None).await
    }

    pub async fn create_subtask(
        &self,
        title: &str,
        workspace_id: i64,
        parent_task_id: i64,
    ) -> anyhow::Result<i64> {
        self.create_task_request(title, workspace_id, Some(parent_task_id))
            .await
    }

    pub async fn toggle_task_completion(&self, task_id: i64) -> anyhow::Result<()> {
        self.post_empty(&format!("/api/tasks/{task_id}/toggle"))
            .await
    }

    pub async fn archive_completed_tasks(&self, workspace_id: i64) -> anyhow::Result<()> {
        self.post_empty(&format!("/api/workspaces/{workspace_id}/archive-completed"))
            .await
    }

    pub async fn update_workspace_name(&self, workspace_id: i64, name: &str) -> anyhow::Result<()> {
        self.http
            .patch(self.url(&format!("/api/workspaces/{workspace_id}")))
            .json(&UpdateWorkspaceRequest {
                name: name.to_string(),
            })
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    pub async fn update_task_name(&self, task_id: i64, title: &str) -> anyhow::Result<()> {
        self.update_task(
            task_id,
            &UpdateTaskRequest {
                title: Some(title.to_string()),
                due_date: None,
                due_date_set: false,
            },
        )
        .await
    }

    pub async fn update_task_due_date(
        &self,
        task_id: i64,
        due_date: Option<&str>,
    ) -> anyhow::Result<()> {
        self.update_task(
            task_id,
            &UpdateTaskRequest {
                title: None,
                due_date: due_date.map(ToString::to_string),
                due_date_set: true,
            },
        )
        .await
    }

    pub async fn delete_workspace(&self, workspace_id: i64) -> anyhow::Result<()> {
        self.http
            .delete(self.url(&format!("/api/workspaces/{workspace_id}")))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    pub async fn delete_task(&self, task_id: i64) -> anyhow::Result<()> {
        self.http
            .delete(self.url(&format!("/api/tasks/{task_id}")))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn create_task_request(
        &self,
        title: &str,
        workspace_id: i64,
        parent_task_id: Option<i64>,
    ) -> anyhow::Result<i64> {
        let response = self
            .http
            .post(self.url(&format!("/api/workspaces/{workspace_id}/tasks")))
            .json(&CreateTaskRequest {
                title: title.to_string(),
                parent_task_id,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<IdResponse>()
            .await?;

        Ok(response.id)
    }

    async fn update_task(&self, task_id: i64, payload: &UpdateTaskRequest) -> anyhow::Result<()> {
        self.http
            .patch(self.url(&format!("/api/tasks/{task_id}")))
            .json(payload)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn post_empty(&self, path: &str) -> anyhow::Result<()> {
        self.http
            .post(self.url(path))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.endpoint_url, path)
    }
}

#[derive(Deserialize)]
struct WorkspacesResponse {
    workspaces: Vec<Workspace>,
    stats: Vec<WorkspaceStats>,
}

#[derive(Deserialize)]
struct IdResponse {
    id: i64,
}

#[derive(Serialize)]
struct CreateWorkspaceRequest {
    name: String,
}

#[derive(Serialize)]
struct UpdateWorkspaceRequest {
    name: String,
}

#[derive(Serialize)]
struct CreateTaskRequest {
    title: String,
    parent_task_id: Option<i64>,
}

#[derive(Serialize)]
struct UpdateTaskRequest {
    title: Option<String>,
    due_date: Option<String>,
    due_date_set: bool,
}
