#[tokio::main]
async fn main() -> anyhow::Result<()> {
    eclipse_ibc_cli::run().await?;
    Ok(())
}
