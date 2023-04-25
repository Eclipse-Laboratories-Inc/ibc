use {
    crate::{generate, query, tx},
    clap::{Parser, Subcommand},
};

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
enum CliSubcommand {
    Generate(generate::Args),
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
        CliSubcommand::Generate(sub_args) => generate::run(sub_args).await,
        CliSubcommand::Query(sub_args) => query::run(sub_args).await,
        CliSubcommand::Tx(sub_args) => tx::run(sub_args).await,
    }
}
