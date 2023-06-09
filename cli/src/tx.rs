use {
    anyhow::anyhow,
    borsh::BorshSerialize,
    clap::{Parser, Subcommand},
    eclipse_ibc_known_proto::{KnownAnyProto, KnownProto},
    eclipse_ibc_program::{
        ibc_contract_instruction::IbcContractInstruction,
        ibc_instruction::msgs::{
            MsgBindPort, MsgInitStorageAccount, MsgReleasePort, MsgWriteTxBuffer,
            MsgWriteTxBufferMode,
        },
    },
    ibc::core::ics24_host::identifier::PortId,
    ibc_proto::{
        google::protobuf,
        ibc::core::{
            channel::v1::{
                MsgChannelCloseConfirm as RawMsgChannelCloseConfirm,
                MsgChannelCloseInit as RawMsgChannelCloseInit,
                MsgChannelOpenAck as RawMsgChannelOpenAck,
                MsgChannelOpenConfirm as RawMsgChannelOpenConfirm,
                MsgChannelOpenInit as RawMsgChannelOpenInit,
                MsgChannelOpenTry as RawMsgChannelOpenTry,
            },
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
    log::info,
    serde::de::DeserializeOwned,
    solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig},
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        message::Message,
        pubkey::Pubkey,
        signer::{
            keypair::{read_keypair_file, Keypair},
            Signer as _,
        },
        system_program,
        sysvar::{clock, rent},
        transaction::Transaction,
    },
    std::{
        io::{self, BufReader},
        path::PathBuf,
        sync::Arc,
    },
};

// Setting `skip_preflight: true` lets us see `ic_msg` log messages for failed txs.
const RPC_SEND_TRANSACTION_CONFIG: RpcSendTransactionConfig = RpcSendTransactionConfig {
    skip_preflight: true,
    preflight_commitment: None,
    encoding: None,
    max_retries: None,
    min_context_slot: None,
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

fn stdin_json_to_any<T>(
    type_url: &str,
    modify_msg: impl FnOnce(&mut T),
) -> anyhow::Result<protobuf::Any>
where
    T: DeserializeOwned + prost::Message,
{
    let mut msg = serde_json::from_reader(BufReader::new(io::stdin()))?;
    modify_msg(&mut msg);

    Ok(protobuf::Any {
        type_url: type_url.to_owned(),
        value: msg.encode_to_vec(),
    })
}

#[derive(Clone, Debug, Subcommand)]
enum ChannelTx {
    OpenInit,
    OpenTry,
    OpenAck,
    OpenConfirm,
    CloseInit,
    CloseConfirm,
}

impl ChannelTx {
    fn encode_as_any(&self, signer: ibc::Signer) -> anyhow::Result<protobuf::Any> {
        match self {
            Self::OpenInit => stdin_json_to_any::<RawMsgChannelOpenInit>(
                "/ibc.core.channel.v1.MsgChannelOpenInit",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
            Self::OpenTry => stdin_json_to_any::<RawMsgChannelOpenTry>(
                "/ibc.core.channel.v1.MsgChannelOpenTry",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
            Self::OpenAck => stdin_json_to_any::<RawMsgChannelOpenAck>(
                "/ibc.core.channel.v1.MsgChannelOpenAck",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
            Self::OpenConfirm => stdin_json_to_any::<RawMsgChannelOpenConfirm>(
                "/ibc.core.channel.v1.MsgChannelOpenConfirm",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
            Self::CloseInit => stdin_json_to_any::<RawMsgChannelCloseInit>(
                "/ibc.core.channel.v1.MsgChannelCloseInit",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
            Self::CloseConfirm => stdin_json_to_any::<RawMsgChannelCloseConfirm>(
                "/ibc.core.channel.v1.MsgChannelCloseConfirm",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
        }
    }
}

#[derive(Clone, Debug, Subcommand)]
enum ClientTx {
    Create,
    Update,
    Misbehaviour,
    Upgrade,
}

impl ClientTx {
    fn encode_as_any(&self, signer: ibc::Signer) -> anyhow::Result<protobuf::Any> {
        match self {
            Self::Create => stdin_json_to_any::<RawMsgCreateClient>(
                "/ibc.core.client.v1.MsgCreateClient",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
            Self::Update => stdin_json_to_any::<RawMsgUpdateClient>(
                "/ibc.core.client.v1.MsgUpdateClient",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
            Self::Misbehaviour => stdin_json_to_any::<RawMsgSubmitMisbehaviour>(
                "/ibc.core.client.v1.MsgSubmitMisbehaviour",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
            Self::Upgrade => stdin_json_to_any::<RawMsgUpgradeClient>(
                "/ibc.core.client.v1.MsgUpgradeClient",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Debug, Subcommand)]
enum ConnectionTx {
    OpenInit,
    OpenTry,
    OpenAck,
    OpenConfirm,
}

impl ConnectionTx {
    fn encode_as_any(&self, signer: ibc::Signer) -> anyhow::Result<protobuf::Any> {
        match self {
            Self::OpenInit => stdin_json_to_any::<RawMsgConnectionOpenInit>(
                "/ibc.core.connection.v1.MsgConnectionOpenInit",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
            Self::OpenTry => stdin_json_to_any::<RawMsgConnectionOpenTry>(
                "/ibc.core.connection.v1.MsgConnectionOpenTry",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
            Self::OpenAck => stdin_json_to_any::<RawMsgConnectionOpenAck>(
                "/ibc.core.connection.v1.MsgConnectionOpenAck",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
            Self::OpenConfirm => stdin_json_to_any::<RawMsgConnectionOpenConfirm>(
                "/ibc.core.connection.v1.MsgConnectionOpenConfirm",
                |msg| {
                    msg.signer = signer.to_string();
                },
            ),
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
    Channel(ChannelTx),
    #[command(subcommand)]
    Client(ClientTx),
    #[command(subcommand)]
    Connection(ConnectionTx),
    #[command(subcommand)]
    Port(PortTx),
}

impl TxKind {
    fn encode_as_any(&self, signer: ibc::Signer) -> anyhow::Result<protobuf::Any> {
        match self {
            Self::Admin(tx) => Ok(tx.encode_as_any()),
            Self::Channel(tx) => tx.encode_as_any(signer),
            Self::Client(tx) => tx.encode_as_any(signer),
            Self::Connection(tx) => tx.encode_as_any(signer),
            Self::Port(tx) => Ok(tx.encode_as_any()),
        }
    }

    fn instruction_data(&self, payer_key: Pubkey) -> anyhow::Result<Vec<u8>> {
        let signer = payer_key.to_string().into();
        Ok(self.encode_as_any(signer)?.encode())
    }

    fn accounts(&self, payer_key: Pubkey) -> Vec<AccountMeta> {
        match self {
            Self::Admin(_) => vec![
                AccountMeta::new_readonly(payer_key, true),
                AccountMeta::new(eclipse_ibc_program::STORAGE_KEY, false),
                AccountMeta::new_readonly(rent::id(), false),
                AccountMeta::new_readonly(clock::id(), false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            Self::Channel(_) | Self::Client(_) | Self::Connection(_) | Self::Port(_) => {
                vec![
                    AccountMeta::new_readonly(payer_key, true),
                    AccountMeta::new(eclipse_ibc_program::STORAGE_KEY, false),
                    AccountMeta::new_readonly(clock::id(), false),
                ]
            }
        }
    }
}

const MAX_SINGLE_INSTRUCTION_SIZE: usize = 825;

async fn split_ibc_instruction_across_txs(
    mut ibc_instruction_data: Vec<u8>,
    payer: &Arc<Keypair>,
    kind: &TxKind,
) -> anyhow::Result<Vec<(Message, Vec<Arc<Keypair>>)>> {
    let payer_key = payer.pubkey();

    let mut messages = vec![];
    let mut buffer_pubkeys = vec![];
    while ibc_instruction_data.len() > MAX_SINGLE_INSTRUCTION_SIZE {
        let new_ibc_instruction_data = ibc_instruction_data.split_off(MAX_SINGLE_INSTRUCTION_SIZE);
        let split_instruction_data = ibc_instruction_data;
        ibc_instruction_data = new_ibc_instruction_data;

        let to_keypair = Keypair::new();
        let to_pubkey = to_keypair.pubkey();

        buffer_pubkeys.push(to_pubkey);

        // TODO: Create a bigger buffer and write to it multiple times, instead of creating
        // a new buffer for each chunk.
        let ibc_instruction_data = MsgWriteTxBuffer {
            mode: MsgWriteTxBufferMode::Create {
                buffer_size: MAX_SINGLE_INSTRUCTION_SIZE.try_into()?,
            },
            data: split_instruction_data,
        }
        .encode_as_any()
        .encode();

        let instruction_data = BorshSerialize::try_to_vec(&IbcContractInstruction {
            extra_accounts_for_instruction: 0,
            last_instruction_part: ibc_instruction_data,
        })?;

        let instructions = [Instruction::new_with_bytes(
            eclipse_ibc_program::id(),
            &instruction_data,
            vec![
                AccountMeta::new_readonly(payer_key, true),
                AccountMeta::new(to_pubkey, true),
                AccountMeta::new_readonly(rent::id(), false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
        )];

        let message = Message::new(&instructions, Some(&payer_key));
        messages.push((message, vec![Arc::clone(payer), Arc::new(to_keypair)]));
    }

    let buffer_accounts = buffer_pubkeys
        .into_iter()
        .map(|buffer_pubkey| AccountMeta::new_readonly(buffer_pubkey, false))
        .collect();

    let instruction_data = BorshSerialize::try_to_vec(&IbcContractInstruction {
        extra_accounts_for_instruction: messages.len(),
        last_instruction_part: ibc_instruction_data,
    })?;

    let main_instruction = Instruction::new_with_bytes(
        eclipse_ibc_program::id(),
        &instruction_data,
        [buffer_accounts, kind.accounts(payer_key)].concat(),
    );
    let main_message = Message::new(&[main_instruction], Some(&payer_key));
    messages.push((main_message, vec![Arc::clone(payer)]));

    Ok(messages)
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
    let payer = Arc::new(
        read_keypair_file(&payer)
            .map_err(|err| anyhow!("Error reading keypair file: {:?}", err))?,
    );
    let rpc_client = RpcClient::new(endpoint);

    let messages =
        split_ibc_instruction_across_txs(kind.instruction_data(payer.pubkey())?, &payer, &kind)
            .await?;

    info!("Submitting IBC txs: {kind:?}");
    for (message, keypairs) in messages {
        info!("Submitting message: {message:?}");
        let blockhash = rpc_client.get_latest_blockhash().await?;

        let signers = keypairs
            .iter()
            .map(|keypair| &**keypair)
            .collect::<Vec<&Keypair>>();
        let tx = Transaction::new(&signers, message, blockhash);
        let sig = rpc_client
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx,
                rpc_client.commitment(),
                RPC_SEND_TRANSACTION_CONFIG,
            )
            .await?;

        info!("Submitted IBC tx: {sig}");
    }

    Ok(())
}
