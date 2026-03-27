mod binary;
mod cli;

use binary::{dataset_gen, pretrain, stats};
use clap::Parser;
use cli::{Cli, Command};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli: Cli = Cli::parse();

    match cli.command {
        Command::Stats(subargs) => {
            stats::run(subargs)?;
        }
        Command::Pretrain(subargs) => {
            pretrain::run(subargs).await?;
        }
        Command::DatasetGen(subargs) => {
            dataset_gen::run(subargs).await?;
        }
    }

    Ok(())
}
