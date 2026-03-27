use clap::{Parser, Subcommand};

use crate::binary::{dataset_gen, pretrain, stats};

#[derive(Parser)]
#[command(
    name = "text-to-cypher",
    about = "Cypher generation toolkit",
    version,
    propagate_version = true
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    Stats(stats::SubArgs),
    Pretrain(pretrain::SubArgs),
    DatasetGen(dataset_gen::SubArgs),
}
