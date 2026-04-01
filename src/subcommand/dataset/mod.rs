mod subcommand;

use clap::{Args, Subcommand};
use subcommand::generate;
use subcommand_macro::Dispatch;

use crate::subcommand::dataset::subcommand::evaluate;

#[derive(Args)]
/// Manage datasets for Cypher generation
///
/// Create and evaluate datasets of natural language queries paired with their
/// Cypher equivalents.
pub(crate) struct SubArgs {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand, Dispatch)]
pub(crate) enum Command {
    Generate(generate::SubArgs),
    Evaluate(evaluate::SubArgs),
}

pub(crate) async fn run(args: SubArgs) -> anyhow::Result<()> {
    args.command.dispatch().await
}
