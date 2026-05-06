use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct WorkspacesResponse {
    pub workspaces: Vec<todo_client::Workspace>,
    pub stats: Vec<todo_client::WorkspaceStats>,
}

#[derive(Serialize)]
pub struct IdResponse {
    pub id: i64,
}

#[derive(Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub parent_task_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub due_date: Option<String>,
    #[serde(default)]
    pub due_date_set: bool,
}
