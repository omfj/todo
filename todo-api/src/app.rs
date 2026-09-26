use axum::{
    Router,
    routing::{get, patch, post},
};
use tower::ServiceBuilder;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::{
    AppState,
    handlers::{
        archive_completed_tasks, create_task, create_workspace, delete_task, delete_workspace,
        health, list_tasks, list_workspaces, toggle_task, update_task, update_workspace,
    },
};

pub fn router(db: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route(
            "/api/workspaces",
            get(list_workspaces).post(create_workspace),
        )
        .route(
            "/api/workspaces/{workspace_id}",
            patch(update_workspace).delete(delete_workspace),
        )
        .route(
            "/api/workspaces/{workspace_id}/tasks",
            get(list_tasks).post(create_task),
        )
        .route(
            "/api/workspaces/{workspace_id}/archive-completed",
            post(archive_completed_tasks),
        )
        .route(
            "/api/tasks/{task_id}",
            patch(update_task).delete(delete_task),
        )
        .route("/api/tasks/{task_id}/toggle", post(toggle_task))
        .layer(
            ServiceBuilder::new().layer(
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                    .on_request(DefaultOnRequest::new().level(Level::INFO))
                    .on_response(DefaultOnResponse::new().level(Level::INFO)),
            ),
        )
        .with_state(db)
}
