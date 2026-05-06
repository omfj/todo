use todo_client::{Client, CryptoKey};

mod config;
mod ui;
mod ui_helpers;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config = config::load_or_create_config()?;
    let crypto = CryptoKey::from_recovery_phrase(&config.phrase);
    let client = Client::new(config.endpoint, crypto);

    ui::run_app(client).await?;

    Ok(())
}
