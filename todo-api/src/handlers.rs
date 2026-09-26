use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};

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
    headers: HeaderMap,
) -> Result<Json<WorkspacesResponse>, ApiError> {
    let user_id = user_id(&headers)?;
    db.ensure_user(user_id).await?;
    let workspaces = db.get_workspaces(user_id).await?;
    let stats = db.get_workspace_stats(user_id).await?;

    Ok(Json(WorkspacesResponse { workspaces, stats }))
}

pub async fn create_workspace(
    State(db): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateWorkspaceRequest>,
) -> Result<Json<IdResponse>, ApiError> {
    let user_id = user_id(&headers)?;
    db.ensure_user(user_id).await?;
    let id = db.create_workspace(user_id, &payload.name).await?;

    Ok(Json(IdResponse { id }))
}

pub async fn update_workspace(
    State(db): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<i64>,
    Json(payload): Json<UpdateWorkspaceRequest>,
) -> Result<StatusCode, ApiError> {
    let user_id = user_id(&headers)?;
    db.update_workspace_name(user_id, workspace_id, &payload.name)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_workspace(
    State(db): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let user_id = user_id(&headers)?;
    db.delete_workspace(user_id, workspace_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_tasks(
    State(db): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<i64>,
) -> Result<Json<Vec<todo_client::EncryptedTask>>, ApiError> {
    let user_id = user_id(&headers)?;
    let tasks = db.get_tasks_for_workspace(user_id, workspace_id).await?;

    Ok(Json(tasks))
}

pub async fn create_task(
    State(db): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<i64>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<Json<IdResponse>, ApiError> {
    let user_id = user_id(&headers)?;
    db.ensure_user(user_id).await?;
    let id = match payload.parent_task_id {
        Some(parent_task_id) => {
            db.create_subtask(user_id, &payload.title, workspace_id, parent_task_id)
                .await?
        }
        None => {
            db.create_task(user_id, &payload.title, workspace_id)
                .await?
        }
    };

    Ok(Json(IdResponse { id }))
}

pub async fn archive_completed_tasks(
    State(db): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let user_id = user_id(&headers)?;
    db.archive_completed_tasks(user_id, workspace_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn update_task(
    State(db): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<i64>,
    Json(payload): Json<UpdateTaskRequest>,
) -> Result<StatusCode, ApiError> {
    let user_id = user_id(&headers)?;
    if let Some(title) = payload.title {
        db.update_task_name(user_id, task_id, &title).await?;
    }

    if payload.due_date_set {
        db.update_task_due_date(user_id, task_id, payload.due_date.as_ref())
            .await?;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_task(
    State(db): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let user_id = user_id(&headers)?;
    db.delete_task(user_id, task_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn toggle_task(
    State(db): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let user_id = user_id(&headers)?;
    db.toggle_task_completion(user_id, task_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

fn user_id(headers: &HeaderMap) -> Result<&str, ApiError> {
    let user_id = headers
        .get("x-user-id")
        .ok_or_else(|| ApiError::bad_request("missing x-user-id header"))?
        .to_str()
        .map_err(|_| ApiError::bad_request("invalid x-user-id header"))?;

    if user_id.len() != 64 || !user_id.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(ApiError::bad_request("invalid x-user-id header"));
    }

    Ok(user_id)
}
