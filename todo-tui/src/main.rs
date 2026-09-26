use clap::Parser;
use todo_cli::CliCommand;
use todo_core::Database;

mod command;
mod ui;

#[derive(Parser)]
#[command(name = "todo")]
struct Args {
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let db = Database::connect().await?;

    match args.command {
        Some(command) => todo_cli::run(&db, command).await?,
        None => ui::run_app(db).await?,
    }

    Ok(())
}
