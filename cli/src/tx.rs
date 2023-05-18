use {
    anyhow::anyhow,
    borsh::BorshSerialize,
    clap::{Parser, Subcommand},
    eclipse_ibc_known_proto::{KnownAnyProto, KnownProto},
    eclipse_ibc_program::{
        ibc_contract_instruction::IbcContractInstruction,
        ibc_instruction::msgs::{MsgBindPort, MsgInitStorageAccount, MsgReleasePort},
    },
    ed25519_dalek::{PublicKey, SecretKey, SECRET_KEY_LENGTH},
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
    rand::{thread_rng, RngCore},
    serde::de::DeserializeOwned,
    solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig},
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        message::Message,
        pubkey::Pubkey,
        signer::{keypair::read_keypair_file, Signer as _},
        system_instruction, system_program,
        sysvar::{clock, rent},
        transaction::Transaction,
    },
    std::{
        io::{self, BufReader},
        path::PathBuf,
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
        let ibc_instruction_data = self.encode_as_any(signer)?.encode();

        // TODO: Split the tx into multiple txs when necessary
        let ibc_contract_instruction = IbcContractInstruction {
            extra_accounts_for_instruction: 0,
            last_instruction_part: ibc_instruction_data,
        };

        Ok(BorshSerialize::try_to_vec(&ibc_contract_instruction)?)
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

const MAX_SINGLE_INSTRUCTION_SIZE: usize = 1000;
const SHARED_MEMORY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    12, 253, 35, 153, 192, 26, 162, 149, 165, 88, 176, 92, 161, 227, 92, 88, 237, 237, 116, 169,
    49, 186, 124, 227, 162, 155, 104, 10, 255, 31, 248, 49,
]);

fn generate_secret_key() -> SecretKey {
    let mut secret_key_bytes = [0u8; SECRET_KEY_LENGTH];
    thread_rng().fill_bytes(&mut secret_key_bytes);
    SecretKey::from_bytes(&secret_key_bytes).unwrap()
}

async fn split_ibc_instruction_across_txs(
    mut ibc_instruction_data: Vec<u8>,
    rpc_client: &RpcClient,
    payer_pubkey: Pubkey,
    kind: &TxKind,
) -> anyhow::Result<Vec<Message>> {
    let rent_exempt_lamports = rpc_client
        .get_minimum_balance_for_rent_exemption(MAX_SINGLE_INSTRUCTION_SIZE)
        .await?;

    let mut messages = vec![];
    let mut buffer_pubkeys = vec![];
    while ibc_instruction_data.len() > MAX_SINGLE_INSTRUCTION_SIZE {
        let new_ibc_instruction_data = ibc_instruction_data.split_off(MAX_SINGLE_INSTRUCTION_SIZE);
        let split_instruction_data = ibc_instruction_data;
        ibc_instruction_data = new_ibc_instruction_data;

        let to_secret_key = generate_secret_key();
        let to_public_key = PublicKey::from(&to_secret_key);
        let to_pubkey = Pubkey::new_from_array(to_public_key.to_bytes());

        buffer_pubkeys.push(to_pubkey);

        // The shared memory program uses the first 8 bytes of the instruction as a little-endian
        // offset into the data.
        let shared_memory_instruction_data = [vec![0u8; 8], split_instruction_data].concat();

        let instructions = [
            system_instruction::create_account(
                &payer_pubkey,
                &to_pubkey,
                rent_exempt_lamports,
                MAX_SINGLE_INSTRUCTION_SIZE.try_into()?,
                &payer_pubkey,
            ),
            Instruction::new_with_bytes(
                SHARED_MEMORY_PROGRAM_ID,
                &shared_memory_instruction_data,
                vec![AccountMeta::new(to_pubkey, false)],
            ),
        ];

        let message = Message::new(&instructions, Some(&payer_pubkey));
        messages.push(message);
    }

    let buffer_accounts = buffer_pubkeys
        .into_iter()
        .map(|buffer_pubkey| AccountMeta::new_readonly(buffer_pubkey, false))
        .collect();

    let main_instruction = Instruction::new_with_bytes(
        eclipse_ibc_program::id(),
        &ibc_instruction_data,
        [buffer_accounts, kind.accounts(payer_pubkey)].concat(),
    );
    let main_message = Message::new(&[main_instruction], Some(&payer_pubkey));
    messages.push(main_message);

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
    let payer = read_keypair_file(&payer)
        .map_err(|err| anyhow!("Error reading keypair file: {:?}", err))?;
    let rpc_client = RpcClient::new(endpoint);

    let messages = split_ibc_instruction_across_txs(
        kind.instruction_data(payer.pubkey())?,
        &rpc_client,
        payer.pubkey(),
        &kind,
    )
    .await?;

    info!("Submitting IBC txs: {kind:?}");
    for message in messages {
        info!("Submitting message: {message:?}");
        let blockhash = rpc_client.get_latest_blockhash().await?;

        let tx = Transaction::new(&[&payer], message, blockhash);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_memory_program_id_matches_spl() {
        let expected_program_id: Pubkey = "shmem4EWT2sPdVGvTZCzXXRAURL9G5vpPxNwSeKhHUL"
            .parse()
            .unwrap();
        assert_eq!(expected_program_id, SHARED_MEMORY_PROGRAM_ID);
    }
}
