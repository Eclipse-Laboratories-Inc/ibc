use {
    clap::{builder::ValueParser, Parser},
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        message::Message,
        signer::{keypair::Keypair, Signer},
        sysvar::{clock, slot_hashes},
        transaction::Transaction,
    },
};

fn parse_bs58_str(s: &str) -> bs58::decode::Result<Vec<u8>> {
    bs58::decode(s).into_vec()
}

#[derive(Debug, Parser)]
pub(crate) struct Args {
    /// Endpoint to send a request to
    #[arg(long, default_value = "http://127.0.0.1:8899")]
    endpoint: String,

    /// Keypair for payer, as a base58-encoded string
    #[arg(long, env, value_parser = ValueParser::new(parse_bs58_str))]
    payer: Vec<u8>,

    /// Instruction to submit
    instruction_data: Vec<u8>,
}

pub(crate) async fn run(
    Args {
        endpoint,
        payer,
        instruction_data,
    }: Args,
) -> anyhow::Result<()> {
    let payer = Keypair::from_bytes(&payer)?;
    let rpc_client = RpcClient::new(endpoint);

    let instruction = Instruction::new_with_bytes(
        eclipse_ibc_program::id(),
        &instruction_data,
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(eclipse_ibc_program::STORAGE_KEY, false),
            AccountMeta::new_readonly(clock::id(), false),
            AccountMeta::new_readonly(slot_hashes::id(), false),
        ],
    );

    let message = Message::new(&[instruction], Some(&payer.pubkey()));
    let blockhash = rpc_client.get_latest_blockhash().await?;

    let tx = Transaction::new(&[&payer], message, blockhash);
    let sig = rpc_client
        .send_and_confirm_transaction_with_spinner(&tx)
        .await?;

    println!("Submitted IBC tx: {sig}");

    Ok(())
}
