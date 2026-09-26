use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct WorkspacesResponse {
    pub workspaces: Vec<todo_client::EncryptedWorkspace>,
    pub stats: Vec<todo_client::WorkspaceStats>,
}

#[derive(Serialize)]
pub struct IdResponse {
    pub id: i64,
}

#[derive(Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: todo_client::EncryptedField,
}

#[derive(Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub name: todo_client::EncryptedField,
}

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub title: todo_client::EncryptedField,
    pub parent_task_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<todo_client::EncryptedField>,
    pub due_date: Option<todo_client::EncryptedField>,
    #[serde(default)]
    pub due_date_set: bool,
}
