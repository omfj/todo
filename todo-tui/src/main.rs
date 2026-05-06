use todo_client::Client;

mod ui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let endpoint_url =
        std::env::var("ENDPOINT_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
    let client = Client::new(endpoint_url);

    ui::run_app(client).await?;

    Ok(())
}
