use axum::{Json, extract::Path, extract::State, http::StatusCode};

use crate::{
    AppState,
    dto::{
        CreateTaskRequest, CreateWorkspaceRequest, IdResponse, UpdateTaskRequest,
        UpdateWorkspaceRequest, WorkspacesResponse,
    },
    error::ApiError,
};

pub async fn health() -> &'static str {
    "ok"
}

pub async fn list_workspaces(
    State(db): State<AppState>,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let workspaces = db.get_workspaces().await?;
    let stats = db.get_workspace_stats().await?;

    Ok(Json(WorkspacesResponse { workspaces, stats }))
}

pub async fn create_workspace(
    State(db): State<AppState>,
    Json(payload): Json<CreateWorkspaceRequest>,
) -> Result<Json<IdResponse>, ApiError> {
    let id = db.create_workspace(payload.name.trim()).await?;

    Ok(Json(IdResponse { id }))
}

pub async fn update_workspace(
    State(db): State<AppState>,
    Path(workspace_id): Path<i64>,
    Json(payload): Json<UpdateWorkspaceRequest>,
) -> Result<StatusCode, ApiError> {
    db.update_workspace_name(workspace_id, payload.name.trim())
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_workspace(
    State(db): State<AppState>,
    Path(workspace_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    db.delete_workspace(workspace_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_tasks(
    State(db): State<AppState>,
    Path(workspace_id): Path<i64>,
) -> Result<Json<Vec<todo_client::Task>>, ApiError> {
    let tasks = db.get_tasks_for_workspace(workspace_id).await?;

    Ok(Json(tasks))
}

pub async fn create_task(
    State(db): State<AppState>,
    Path(workspace_id): Path<i64>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<Json<IdResponse>, ApiError> {
    let title = payload.title.trim();
    let id = match payload.parent_task_id {
        Some(parent_task_id) => {
            db.create_subtask(title, workspace_id, parent_task_id)
                .await?
        }
        None => db.create_task(title, workspace_id).await?,
    };

    Ok(Json(IdResponse { id }))
}

pub async fn archive_completed_tasks(
    State(db): State<AppState>,
    Path(workspace_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    db.archive_completed_tasks(workspace_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn update_task(
    State(db): State<AppState>,
    Path(task_id): Path<i64>,
    Json(payload): Json<UpdateTaskRequest>,
) -> Result<StatusCode, ApiError> {
    if let Some(title) = payload.title {
        db.update_task_name(task_id, title.trim()).await?;
    }

    if payload.due_date_set {
        db.update_task_due_date(task_id, payload.due_date.as_deref())
            .await?;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_task(
    State(db): State<AppState>,
    Path(task_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    db.delete_task(task_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn toggle_task(
    State(db): State<AppState>,
    Path(task_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    db.toggle_task_completion(task_id).await?;

    Ok(StatusCode::NO_CONTENT)
}
