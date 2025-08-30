use crate::db::Db;

mod db;
mod models;
mod ui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Db::connect().await?;

    ui::run_app(db).await?;

    Ok(())
}
