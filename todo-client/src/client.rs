use crate::{
    CryptoKey, EncryptedField, EncryptedTask, EncryptedWorkspace, Task, Workspace, WorkspaceStats,
};
use serde::{Deserialize, Serialize};

const USER_ID_HEADER: &str = "x-user-id";

#[derive(Debug, Clone)]
pub struct Client {
    endpoint_url: String,
    http: reqwest::Client,
    crypto: CryptoKey,
}

impl Client {
    pub fn new(endpoint_url: String, crypto: CryptoKey) -> Self {
        Self {
            endpoint_url: normalize_endpoint_url(&endpoint_url),
            http: reqwest::Client::new(),
            crypto,
        }
    }

    pub async fn get_workspaces(&self) -> anyhow::Result<Vec<Workspace>> {
        let response = self
            .with_user(self.http.get(self.url("/api/workspaces")))
            .send()
            .await?
            .error_for_status()?
            .json::<WorkspacesResponse>()
            .await?;

        response
            .workspaces
            .into_iter()
            .map(|workspace| self.decrypt_workspace(workspace))
            .collect()
    }

    pub async fn get_workspace_stats(&self) -> anyhow::Result<Vec<WorkspaceStats>> {
        let response = self
            .with_user(self.http.get(self.url("/api/workspaces")))
            .send()
            .await?
            .error_for_status()?
            .json::<WorkspacesResponse>()
            .await?;

        Ok(response.stats)
    }

    pub async fn get_tasks_for_workspace(&self, workspace_id: i64) -> anyhow::Result<Vec<Task>> {
        let tasks = self
            .with_user(
                self.http
                    .get(self.url(&format!("/api/workspaces/{workspace_id}/tasks"))),
            )
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<EncryptedTask>>()
            .await?;

        tasks
            .into_iter()
            .map(|task| self.decrypt_task(task))
            .collect()
    }

    pub async fn create_workspace(&self, name: &str) -> anyhow::Result<i64> {
        let response = self
            .with_user(self.http.post(self.url("/api/workspaces")))
            .json(&CreateWorkspaceRequest {
                name: self.crypto.encrypt_field(name)?,
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
        self.with_user(
            self.http
                .patch(self.url(&format!("/api/workspaces/{workspace_id}"))),
        )
        .json(&UpdateWorkspaceRequest {
            name: self.crypto.encrypt_field(name)?,
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
                title: Some(self.crypto.encrypt_field(title)?),
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
                due_date: due_date
                    .map(|due_date| self.crypto.encrypt_field(due_date))
                    .transpose()?,
                due_date_set: true,
            },
        )
        .await
    }

    pub async fn delete_workspace(&self, workspace_id: i64) -> anyhow::Result<()> {
        self.with_user(
            self.http
                .delete(self.url(&format!("/api/workspaces/{workspace_id}"))),
        )
        .send()
        .await?
        .error_for_status()?;

        Ok(())
    }

    pub async fn delete_task(&self, task_id: i64) -> anyhow::Result<()> {
        self.with_user(self.http.delete(self.url(&format!("/api/tasks/{task_id}"))))
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
            .with_user(
                self.http
                    .post(self.url(&format!("/api/workspaces/{workspace_id}/tasks"))),
            )
            .json(&CreateTaskRequest {
                title: self.crypto.encrypt_field(title)?,
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
        self.with_user(self.http.patch(self.url(&format!("/api/tasks/{task_id}"))))
            .json(payload)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn post_empty(&self, path: &str) -> anyhow::Result<()> {
        self.with_user(self.http.post(self.url(path)))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.endpoint_url, path)
    }

    fn with_user(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request.header(USER_ID_HEADER, self.crypto.user_id())
    }

    fn decrypt_workspace(&self, workspace: EncryptedWorkspace) -> anyhow::Result<Workspace> {
        Ok(Workspace {
            id: workspace.id,
            name: self.crypto.decrypt_field(&workspace.name)?,
            created_at: workspace.created_at,
            updated_at: workspace.updated_at,
        })
    }

    fn decrypt_task(&self, task: EncryptedTask) -> anyhow::Result<Task> {
        Ok(Task {
            id: task.id,
            title: self.crypto.decrypt_field(&task.title)?,
            description: task
                .description
                .as_ref()
                .map(|description| self.crypto.decrypt_field(description))
                .transpose()?,
            completed: task.completed,
            archived: task.archived,
            due_date: task
                .due_date
                .as_ref()
                .map(|due_date| self.crypto.decrypt_field(due_date))
                .transpose()?,
            workspace_id: task.workspace_id,
            parent_task_id: task.parent_task_id,
            created_at: task.created_at,
            updated_at: task.updated_at,
        })
    }
}

fn normalize_endpoint_url(endpoint_url: &str) -> String {
    let endpoint_url = endpoint_url.trim().trim_end_matches('/');
    if endpoint_url.starts_with("http://") || endpoint_url.starts_with("https://") {
        endpoint_url.to_string()
    } else {
        format!("https://{endpoint_url}")
    }
}

#[derive(Deserialize)]
struct WorkspacesResponse {
    workspaces: Vec<EncryptedWorkspace>,
    stats: Vec<WorkspaceStats>,
}

#[derive(Deserialize)]
struct IdResponse {
    id: i64,
}

#[derive(Serialize)]
struct CreateWorkspaceRequest {
    name: EncryptedField,
}

#[derive(Serialize)]
struct UpdateWorkspaceRequest {
    name: EncryptedField,
}

#[derive(Serialize)]
struct CreateTaskRequest {
    title: EncryptedField,
    parent_task_id: Option<i64>,
}

#[derive(Serialize)]
struct UpdateTaskRequest {
    title: Option<EncryptedField>,
    due_date: Option<EncryptedField>,
    due_date_set: bool,
}
