use {
    anyhow::anyhow,
    clap::{Parser, Subcommand},
    eclipse_ibc_program::ibc_instruction::msgs::{
        MsgBindPort, MsgInitStorageAccount, MsgReleasePort,
    },
    eclipse_known_proto::{KnownAnyProto, KnownProto},
    ibc::core::ics24_host::identifier::PortId,
    ibc_proto::{
        google::protobuf,
        ibc::core::{
            client::v1::{
                MsgCreateClient as RawMsgCreateClient,
                MsgSubmitMisbehaviour as RawMsgSubmitMisbehaviour,
                MsgUpdateClient as RawMsgUpdateClient, MsgUpgradeClient as RawMsgUpgradeClient,
            },
            connection::v1::{
                MsgConnectionOpenAck as RawMsgConnectionOpenAck,
                MsgConnectionOpenConfirm as RawMsgConnectionOpenConfirm,
                MsgConnectionOpenInit as RawMsgConnectionOpenInit,
                MsgConnectionOpenTry as RawMsgConnectionOpenTry,
            },
        },
    },
    prost::Message as _,
    serde::de::DeserializeOwned,
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        message::Message,
        pubkey::Pubkey,
        signer::{keypair::read_keypair_file, Signer as _},
        system_program,
        sysvar::{clock, rent, slot_hashes},
        transaction::Transaction,
    },
    std::path::PathBuf,
};

fn parse_as_json<T>(s: &str) -> serde_json::Result<T>
where
    T: DeserializeOwned,
{
    serde_json::from_str(s)
}

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

#[derive(Clone, Debug, Subcommand)]
enum ClientTx {
    Create {
        #[arg(value_parser = parse_as_json::<RawMsgCreateClient>)]
        msg: RawMsgCreateClient,
    },
    Update {
        #[arg(value_parser = parse_as_json::<RawMsgUpdateClient>)]
        msg: RawMsgUpdateClient,
    },
    Misbehaviour {
        #[arg(value_parser = parse_as_json::<RawMsgSubmitMisbehaviour>)]
        msg: RawMsgSubmitMisbehaviour,
    },
    Upgrade {
        #[arg(value_parser = parse_as_json::<RawMsgUpgradeClient>)]
        msg: RawMsgUpgradeClient,
    },
}

impl ClientTx {
    fn encode_as_any(&self, signer: ibc::signer::Signer) -> protobuf::Any {
        match self {
            Self::Create { msg } => {
                let msg = RawMsgCreateClient {
                    signer: signer.to_string(),
                    ..msg.clone()
                };
                protobuf::Any {
                    type_url: "/ibc.core.client.v1.MsgCreateClient".to_owned(),
                    value: msg.encode_to_vec(),
                }
            }
            Self::Update { msg } => {
                let msg = RawMsgUpdateClient {
                    signer: signer.to_string(),
                    ..msg.clone()
                };
                protobuf::Any {
                    type_url: "/ibc.core.client.v1.MsgUpdateClient".to_owned(),
                    value: msg.encode_to_vec(),
                }
            }
            Self::Misbehaviour { msg } => {
                let msg = RawMsgSubmitMisbehaviour {
                    signer: signer.to_string(),
                    ..msg.clone()
                };
                protobuf::Any {
                    type_url: "/ibc.core.client.v1.MsgSubmitMisbehaviour".to_owned(),
                    value: msg.encode_to_vec(),
                }
            }
            Self::Upgrade { msg } => {
                let msg = RawMsgUpgradeClient {
                    signer: signer.to_string(),
                    ..msg.clone()
                };
                protobuf::Any {
                    type_url: "/ibc.core.client.v1.MsgUpgradeClient".to_owned(),
                    value: msg.encode_to_vec(),
                }
            }
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Debug, Subcommand)]
enum ConnectionTx {
    OpenInit {
        #[arg(value_parser = parse_as_json::<RawMsgConnectionOpenInit>)]
        msg: RawMsgConnectionOpenInit,
    },
    OpenTry {
        #[arg(value_parser = parse_as_json::<RawMsgConnectionOpenTry>)]
        msg: RawMsgConnectionOpenTry,
    },
    OpenAck {
        #[arg(value_parser = parse_as_json::<RawMsgConnectionOpenAck>)]
        msg: RawMsgConnectionOpenAck,
    },
    OpenConfirm {
        #[arg(value_parser = parse_as_json::<RawMsgConnectionOpenConfirm>)]
        msg: RawMsgConnectionOpenConfirm,
    },
}

impl ConnectionTx {
    fn encode_as_any(&self, signer: ibc::signer::Signer) -> protobuf::Any {
        match self {
            Self::OpenInit { msg } => {
                let msg = RawMsgConnectionOpenInit {
                    signer: signer.to_string(),
                    ..msg.clone()
                };
                protobuf::Any {
                    type_url: "/ibc.core.connection.v1.MsgConnectionOpenInit".to_owned(),
                    value: msg.encode_to_vec(),
                }
            }
            Self::OpenTry { msg } => {
                let msg = RawMsgConnectionOpenTry {
                    signer: signer.to_string(),
                    ..msg.clone()
                };
                protobuf::Any {
                    type_url: "/ibc.core.connection.v1.MsgConnectionOpenTry".to_owned(),
                    value: msg.encode_to_vec(),
                }
            }
            Self::OpenAck { msg } => {
                let msg = RawMsgConnectionOpenAck {
                    signer: signer.to_string(),
                    ..msg.clone()
                };
                protobuf::Any {
                    type_url: "/ibc.core.connection.v1.MsgConnectionOpenAck".to_owned(),
                    value: msg.encode_to_vec(),
                }
            }
            Self::OpenConfirm { msg } => {
                let msg = RawMsgConnectionOpenConfirm {
                    signer: signer.to_string(),
                    ..msg.clone()
                };
                protobuf::Any {
                    type_url: "/ibc.core.connection.v1.MsgConnectionOpenConfirm".to_owned(),
                    value: msg.encode_to_vec(),
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
    Connection(ConnectionTx),
    #[command(subcommand)]
    Port(PortTx),
}

impl TxKind {
    fn encode_as_any(&self, signer: ibc::signer::Signer) -> protobuf::Any {
        match self {
            Self::Admin(tx) => tx.encode_as_any(),
            Self::Client(tx) => tx.encode_as_any(signer),
            Self::Connection(tx) => tx.encode_as_any(signer),
            Self::Port(tx) => tx.encode_as_any(),
        }
    }

    fn instruction_data(&self, payer_key: Pubkey) -> Vec<u8> {
        let signer = payer_key
            .to_string()
            .parse()
            .expect("Pubkey should never be empty");
        self.encode_as_any(signer).encode()
    }

    fn accounts(&self, payer_key: Pubkey) -> Vec<AccountMeta> {
        match self {
            Self::Admin(_) => vec![
                AccountMeta::new_readonly(payer_key, true),
                AccountMeta::new(eclipse_ibc_program::STORAGE_KEY, false),
                AccountMeta::new_readonly(rent::id(), false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            Self::Client(_) | Self::Connection(_) | Self::Port(_) => {
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
