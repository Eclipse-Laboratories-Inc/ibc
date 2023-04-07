use {
    crate::query,
    clap::{Parser, Subcommand},
};

#[derive(Debug, Subcommand)]
enum CliSubcommand {
    Query(query::Args),
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
    }
}
