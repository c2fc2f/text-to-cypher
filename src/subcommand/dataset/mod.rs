mod subcommand;

use clap::{Args, Subcommand};
use subcommand::generate;

#[derive(Args)]
/// Manage datasets for Cypher generation
///
/// Create and evaluate datasets of natural language queries paired with their
/// Cypher equivalents.
pub(crate) struct SubArgs {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    Generate(generate::SubArgs),
}

pub(crate) async fn run(args: SubArgs) -> anyhow::Result<()> {
    match args.command {
        Command::Generate(subargs) => generate::run(subargs).await,
    }?;

    Ok(())
}
