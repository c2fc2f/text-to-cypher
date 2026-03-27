pub mod models;
mod subcommand;

use clap::{Parser, Subcommand};
use subcommand::{dataset, pretrain, stats};
use subcommand_macro::Dispatch;

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

#[derive(Subcommand, Dispatch)]
pub(crate) enum Command {
    Stats(stats::SubArgs),
    Pretrain(pretrain::SubArgs),
    Dataset(dataset::SubArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Cli::parse().command.dispatch().await
}
