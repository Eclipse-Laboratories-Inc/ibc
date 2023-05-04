#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::try_init()?;

    eclipse_ibc_cli::run().await?;
    Ok(())
}
