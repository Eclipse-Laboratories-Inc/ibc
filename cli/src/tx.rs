use {
    anyhow::anyhow,
    clap::{Parser, Subcommand},
    eclipse_ibc_program::ibc_instruction::msgs::{
        MsgBindPort, MsgInitStorageAccount, MsgReleasePort,
    },
    ibc::core::ics24_host::identifier::PortId,
    ibc_proto::google::protobuf,
    known_proto::{KnownAnyProto, KnownProto},
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        message::Message,
        pubkey::Pubkey,
        signer::{keypair::read_keypair_file, Signer},
        sysvar::{clock, rent, slot_hashes},
        transaction::Transaction,
    },
    std::path::PathBuf,
};

#[derive(Clone, Debug, Subcommand)]
enum TxKind {
    BindPort { port_id: PortId },
    InitStorageAccount,
    ReleasePort { port_id: PortId },
}

impl TxKind {
    fn encode_as_any(&self) -> protobuf::Any {
        match self {
            Self::BindPort { port_id } => MsgBindPort {
                port_id: port_id.clone(),
            }
            .encode_as_any(),
            Self::InitStorageAccount => MsgInitStorageAccount.encode_as_any(),
            Self::ReleasePort { port_id } => MsgReleasePort {
                port_id: port_id.clone(),
            }
            .encode_as_any(),
        }
    }

    fn instruction_data(&self) -> Vec<u8> {
        self.encode_as_any().encode()
    }

    fn accounts(&self, payer_key: Pubkey) -> Vec<AccountMeta> {
        match self {
            Self::BindPort { .. } | Self::ReleasePort { .. } => vec![
                AccountMeta::new_readonly(payer_key, true),
                AccountMeta::new(eclipse_ibc_program::STORAGE_KEY, false),
                AccountMeta::new_readonly(clock::id(), false),
                AccountMeta::new_readonly(slot_hashes::id(), false),
            ],
            Self::InitStorageAccount => vec![
                AccountMeta::new_readonly(payer_key, true),
                AccountMeta::new(eclipse_ibc_program::STORAGE_KEY, false),
                AccountMeta::new_readonly(rent::id(), false),
            ],
        }
    }
}

#[derive(Debug, Parser)]
pub(crate) struct Args {
    /// Endpoint to send a request to
    #[arg(long, default_value = "http://127.0.0.1:8899")]
    endpoint: String,

    /// File path to payer keypair
    #[arg(long)]
    payer: Option<PathBuf>,

    /// Transaction kind
    #[command(subcommand)]
    kind: TxKind,
}

pub(crate) async fn run(
    Args {
        endpoint,
        payer,
        kind,
    }: Args,
) -> anyhow::Result<()> {
    let payer = match payer {
        Some(payer) => payer,
        None => {
            let mut keypair_path = dirs_next::home_dir()
                .ok_or_else(|| anyhow!("Could not retrieve home directory"))?;
            keypair_path.extend([".config", "solana", "id.json"]);
            keypair_path
        }
    };
    let payer = read_keypair_file(&payer)
        .map_err(|err| anyhow!("Error reading keypair file: {:?}", err))?;
    let rpc_client = RpcClient::new(endpoint);

    let instruction = Instruction::new_with_bytes(
        eclipse_ibc_program::id(),
        &kind.instruction_data(),
        kind.accounts(payer.pubkey()),
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
