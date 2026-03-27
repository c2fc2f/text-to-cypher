mod subcommand;

use clap::{Parser, Subcommand};
use subcommand::{dataset, pretrain, stats};

#[derive(Parser)]
#[command(version, propagate_version = true)]
/// Benchmark and evaluate NL-to-Cypher techniques
///
/// A CLI for evaluating natural language to Cypher query generation
/// approaches. Provides tools to generate evaluation datasets, run models
/// against them, and visualize performance statistics.
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    Stats(stats::SubArgs),
    Pretrain(pretrain::SubArgs),
    Dataset(dataset::SubArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli: Cli = Cli::parse();

    match cli.command {
        Command::Stats(subargs) => stats::run(subargs).await,
        Command::Pretrain(subargs) => pretrain::run(subargs).await,
        Command::Dataset(subargs) => dataset::run(subargs).await,
    }?;

    Ok(())
}
