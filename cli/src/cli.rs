use {
    crate::{query, tx},
    clap::{Parser, Subcommand},
};

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
enum CliSubcommand {
    Query(query::Args),
    Tx(tx::Args),
}

#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    #[command(subcommand)]
    subcommand: CliSubcommand,
}

pub async fn run() -> anyhow::Result<()> {
    let Args { subcommand } = Args::try_parse()?;

    match subcommand {
        CliSubcommand::Query(query_args) => query::run(query_args).await,
        CliSubcommand::Tx(query_args) => tx::run(query_args).await,
    }
}
