use {
    anyhow::anyhow,
    clap::{builder::ValueParser, Parser, Subcommand},
    eclipse_ibc_light_client::{EclipseClientState, EclipseConsensusState, EclipseHeader},
    eclipse_ibc_program::ibc_instruction::msgs::{
        MsgBindPort, MsgInitStorageAccount, MsgReleasePort,
    },
    ibc::{
        core::{
            ics02_client::{
                height::Height,
                msgs::{
                    create_client::MsgCreateClient,
                    update_client::{self, MsgUpdateClient},
                },
            },
            ics23_commitment::commitment::CommitmentRoot,
            ics24_host::identifier::{ChainId, ClientId, PortId},
        },
        tx_msg::Msg as _,
    },
    ibc_proto::{
        google::protobuf, ibc::core::client::v1::MsgUpdateClient as RawMsgUpdateClient,
        protobuf::Protobuf,
    },
    known_proto::{KnownAnyProto, KnownProto},
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        message::Message,
        pubkey::Pubkey,
        signer::{keypair::read_keypair_file, Signer},
        system_program,
        sysvar::{clock, rent, slot_hashes},
        transaction::Transaction,
    },
    std::path::PathBuf,
    tendermint::time::Time as TendermintTime,
};

#[derive(Clone, Debug, Subcommand)]
enum AdminTx {
    InitStorageAccount,
}

impl AdminTx {
    fn encode_as_any(&self) -> protobuf::Any {
        match self {
            Self::InitStorageAccount => MsgInitStorageAccount.encode_as_any(),
        }
    }
}

fn parse_commitment_root(raw: &str) -> Result<CommitmentRoot, hex::FromHexError> {
    Ok(hex::decode(raw)?.into())
}

#[derive(Clone, Debug, Subcommand)]
enum ClientTx {
    CreateEclipse {
        chain_id: ChainId,
        #[arg(value_parser = ValueParser::new(parse_commitment_root))]
        commitment_root: CommitmentRoot,
        height: Height,
        timestamp: TendermintTime,
    },
    UpdateEclipse {
        client_id: ClientId,
        #[arg(value_parser = ValueParser::new(parse_commitment_root))]
        commitment_root: CommitmentRoot,
        height: Height,
        timestamp: TendermintTime,
    },
}

impl ClientTx {
    fn encode_as_any(&self, payer_key: Pubkey) -> protobuf::Any {
        match self {
            Self::CreateEclipse {
                chain_id,
                commitment_root,
                height,
                timestamp,
            } => {
                let latest_header = EclipseHeader {
                    height: *height,
                    commitment_root: commitment_root.clone(),
                    timestamp: *timestamp,
                };
                let consensus_state = EclipseConsensusState {
                    commitment_root: commitment_root.clone(),
                    timestamp: *timestamp,
                };
                let client_state = EclipseClientState {
                    chain_id: chain_id.clone(),
                    latest_header,
                    frozen_height: None,
                };
                MsgCreateClient::new(
                    client_state.encode_as_any(),
                    consensus_state.encode_as_any(),
                    payer_key
                        .to_string()
                        .parse()
                        .expect("Pubkey should never be empty"),
                )
                .to_any()
            }
            Self::UpdateEclipse {
                client_id,
                commitment_root,
                height,
                timestamp,
            } => {
                let latest_header = EclipseHeader {
                    height: *height,
                    commitment_root: commitment_root.clone(),
                    timestamp: *timestamp,
                };
                let msg = MsgUpdateClient {
                    client_id: client_id.clone(),
                    client_message: latest_header.encode_as_any(),
                    update_kind: update_client::UpdateKind::UpdateClient,
                    signer: payer_key
                        .to_string()
                        .parse()
                        .expect("Pubkey should never be empty"),
                };
                protobuf::Any {
                    type_url: update_client::UPDATE_CLIENT_TYPE_URL.to_owned(),
                    value: <_ as Protobuf<RawMsgUpdateClient>>::encode_vec(&msg).unwrap(),
                }
            }
        }
    }
}

#[derive(Clone, Debug, Subcommand)]
enum PortTx {
    Bind { port_id: PortId },
    Release { port_id: PortId },
}

impl PortTx {
    fn encode_as_any(&self) -> protobuf::Any {
        match self {
            Self::Bind { port_id } => MsgBindPort {
                port_id: port_id.clone(),
            }
            .encode_as_any(),
            Self::Release { port_id } => MsgReleasePort {
                port_id: port_id.clone(),
            }
            .encode_as_any(),
        }
    }
}

#[derive(Clone, Debug, Subcommand)]
enum TxKind {
    #[command(subcommand)]
    Admin(AdminTx),
    #[command(subcommand)]
    Client(ClientTx),
    #[command(subcommand)]
    Port(PortTx),
}

impl TxKind {
    fn encode_as_any(&self, payer_key: Pubkey) -> protobuf::Any {
        match self {
            Self::Admin(admin_tx) => admin_tx.encode_as_any(),
            Self::Client(client_tx) => client_tx.encode_as_any(payer_key),
            Self::Port(port_tx) => port_tx.encode_as_any(),
        }
    }

    fn instruction_data(&self, payer_key: Pubkey) -> Vec<u8> {
        self.encode_as_any(payer_key).encode()
    }

    fn accounts(&self, payer_key: Pubkey) -> Vec<AccountMeta> {
        match self {
            Self::Admin(_) => vec![
                AccountMeta::new_readonly(payer_key, true),
                AccountMeta::new(eclipse_ibc_program::STORAGE_KEY, false),
                AccountMeta::new_readonly(rent::id(), false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            Self::Port(_) | Self::Client(_) => {
                vec![
                    AccountMeta::new_readonly(payer_key, true),
                    AccountMeta::new(eclipse_ibc_program::STORAGE_KEY, false),
                    AccountMeta::new_readonly(clock::id(), false),
                    AccountMeta::new_readonly(slot_hashes::id(), false),
                ]
            }
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
        &kind.instruction_data(payer.pubkey()),
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
