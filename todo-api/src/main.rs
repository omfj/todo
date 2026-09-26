use std::{net::SocketAddr, sync::Arc};

use todo_api::{app, db::Database};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("todo_api=info,tower_http=info")
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db = Arc::new(Database::connect().await?);
    db.run_migrations(&MIGRATOR).await?;

    let app = app::router(db);

    let addr = "0.0.0.0:3000".parse::<SocketAddr>()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "todo-api listening");

    axum::serve(listener, app).await?;

    Ok(())
}
