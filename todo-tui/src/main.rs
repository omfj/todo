use todo_core::Database;

mod command;
mod ui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Database::connect().await?;

    ui::run_app(db).await?;

    Ok(())
}
