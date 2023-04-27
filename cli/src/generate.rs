use {
    crate::chain_state,
    anyhow::anyhow,
    clap::{Parser, Subcommand},
    eclipse_ibc_known_proto::KnownAnyProto,
    eclipse_ibc_light_client::eclipse_chain,
    eclipse_ibc_state::{IbcAccountData, IbcState, IbcStore},
    ibc::core::ics24_host::path::{ClientStatePath, ConnectionPath},
    ibc_proto::ibc::core::{
        client::v1::{
            MsgCreateClient as RawMsgCreateClient, MsgUpdateClient as RawMsgUpdateClient,
            MsgUpgradeClient as RawMsgUpgradeClient,
        },
        commitment::v1::MerklePrefix as RawMerklePrefix,
        connection::v1::{
            Counterparty as RawCounterparty, MsgConnectionOpenAck as RawMsgConnectionOpenAck,
            MsgConnectionOpenConfirm as RawMsgConnectionOpenConfirm,
            MsgConnectionOpenInit as RawMsgConnectionOpenInit,
            MsgConnectionOpenTry as RawMsgConnectionOpenTry,
        },
    },
    serde::Serialize,
    solana_client::nonblocking::rpc_client::RpcClient,
    std::io::{self, Write as _},
};

const DELAY_PERIOD_NANOS: u64 = 0;

async fn get_ibc_store(rpc_client: &RpcClient) -> anyhow::Result<IbcStore> {
    let raw_account_data = rpc_client
        .get_account_data(&eclipse_ibc_program::STORAGE_KEY)
        .await?;

    let IbcAccountData {
        store: ibc_store, ..
    } = bincode::deserialize(&raw_account_data)?;

    Ok(ibc_store)
}

fn get_ibc_state(ibc_store: &IbcStore) -> anyhow::Result<IbcState> {
    let latest_version = ibc_store
        .read()?
        .latest_version()
        .ok_or_else(|| anyhow!("IBC store is missing latest version"))?;

    Ok(IbcState::new(ibc_store, latest_version))
}

fn print_json<T>(msg: T) -> anyhow::Result<()>
where
    T: Serialize,
{
    let json_str = colored_json::to_colored_json_auto(&serde_json::to_value(msg)?)?;
    writeln!(io::stdout(), "{json_str}")?;
    Ok(())
}

#[derive(Clone, Debug, Subcommand)]
enum ClientMsg {
    Create {
        chain_name: String,
    },
    Update {
        client_id: String,
    },
    Upgrade {
        chain_name: String,
        client_id: String,
    },
}

impl ClientMsg {
    async fn generate(&self, rpc_client: &RpcClient) -> anyhow::Result<()> {
        match self {
            Self::Create { chain_name } => {
                let latest_slot = rpc_client.get_slot().await?;
                let latest_height = eclipse_chain::height_of_slot(latest_slot)?;
                let consensus_state =
                    chain_state::get_consensus_state(rpc_client, latest_height).await?;

                let latest_header = chain_state::header_from_consensus_state(
                    consensus_state.clone(),
                    latest_height,
                );
                let client_state = chain_state::client_state_from_header(latest_header, chain_name);

                let msg = RawMsgCreateClient {
                    client_state: Some(client_state.encode_as_any()),
                    consensus_state: Some(consensus_state.encode_as_any()),
                    signer: "".to_owned(),
                };

                print_json(msg)?;
                Ok(())
            }
            Self::Update { client_id } => {
                let latest_slot = rpc_client.get_slot().await?;
                let latest_height = eclipse_chain::height_of_slot(latest_slot)?;
                let consensus_state =
                    chain_state::get_consensus_state(rpc_client, latest_height).await?;
                let latest_header =
                    chain_state::header_from_consensus_state(consensus_state, latest_height);

                let msg = RawMsgUpdateClient {
                    client_id: client_id.clone(),
                    header: Some(latest_header.encode_as_any()),
                    signer: "".to_owned(),
                };

                print_json(msg)?;
                Ok(())
            }
            Self::Upgrade {
                chain_name,
                client_id,
            } => {
                let latest_slot = rpc_client.get_slot().await?;
                let latest_height = eclipse_chain::height_of_slot(latest_slot)?;
                let consensus_state =
                    chain_state::get_consensus_state(rpc_client, latest_height).await?;

                let latest_header = chain_state::header_from_consensus_state(
                    consensus_state.clone(),
                    latest_height,
                );
                let client_state = chain_state::client_state_from_header(latest_header, chain_name);

                let msg = RawMsgUpgradeClient {
                    client_id: client_id.clone(),
                    client_state: Some(client_state.encode_as_any()),
                    consensus_state: Some(consensus_state.encode_as_any()),
                    proof_upgrade_client: vec![],
                    proof_upgrade_consensus_state: vec![],
                    signer: "".to_owned(),
                };

                print_json(msg)?;
                Ok(())
            }
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Debug, Subcommand)]
enum ConnectionMsg {
    OpenInit {
        client_id: String,
    },
    OpenTry {
        client_id: String,
    },
    OpenAck {
        client_id: String,
        connection_id: String,
    },
    OpenConfirm {
        connection_id: String,
    },
}

impl ConnectionMsg {
    async fn generate(&self, rpc_client: &RpcClient) -> anyhow::Result<()> {
        match self {
            Self::OpenInit { client_id } => {
                let counterparty = RawCounterparty {
                    client_id: client_id.to_owned(),
                    connection_id: "".to_owned(),
                    prefix: Some(RawMerklePrefix {
                        key_prefix: eclipse_chain::COMMITMENT_PREFIX.to_vec(),
                    }),
                };

                let msg = RawMsgConnectionOpenInit {
                    client_id: client_id.to_owned(),
                    counterparty: Some(counterparty),
                    version: None,
                    delay_period: DELAY_PERIOD_NANOS,
                    signer: "".to_owned(),
                };

                print_json(msg)?;
                Ok(())
            }
            Self::OpenTry { client_id } => {
                let counterparty = RawCounterparty {
                    client_id: client_id.to_owned(),
                    connection_id: "".to_owned(),
                    prefix: Some(RawMerklePrefix {
                        key_prefix: eclipse_chain::COMMITMENT_PREFIX.to_vec(),
                    }),
                };

                let ibc_store = get_ibc_store(rpc_client).await?;
                let ibc_state = get_ibc_state(&ibc_store)?;

                let client_state = ibc_state.get_raw(&ClientStatePath::new(&client_id.parse()?))?;

                // TODO: Add commitment proofs
                #[allow(deprecated)]
                let msg = RawMsgConnectionOpenTry {
                    client_id: client_id.to_owned(),
                    previous_connection_id: "".to_owned(),
                    client_state,
                    counterparty: Some(counterparty),
                    delay_period: DELAY_PERIOD_NANOS,
                    counterparty_versions: vec![],
                    proof_height: None,
                    proof_init: vec![],
                    proof_client: vec![],
                    proof_consensus: vec![],
                    consensus_height: None,
                    signer: "".to_owned(),
                };

                print_json(msg)?;
                Ok(())
            }
            Self::OpenAck {
                client_id,
                connection_id,
            } => {
                let ibc_store = get_ibc_store(rpc_client).await?;
                let ibc_state = get_ibc_state(&ibc_store)?;

                let client_state = ibc_state.get_raw(&ClientStatePath::new(&client_id.parse()?))?;
                let connection_end = ibc_state
                    .get_raw(&ConnectionPath::new(&client_id.parse()?))?
                    .ok_or_else(|| {
                        anyhow!("Connection does not exist for client ID: {client_id}")
                    })?;
                let counterparty = connection_end.counterparty.ok_or_else(|| {
                    anyhow!("Counterparty does not exist for client ID: {client_id}")
                })?;

                // TODO: Add commitment proofs
                let msg = RawMsgConnectionOpenAck {
                    connection_id: connection_id.to_owned(),
                    counterparty_connection_id: counterparty.connection_id,
                    version: None,
                    client_state,
                    proof_height: None,
                    proof_try: vec![],
                    proof_client: vec![],
                    proof_consensus: vec![],
                    consensus_height: None,
                    signer: "".to_owned(),
                };

                print_json(msg)?;
                Ok(())
            }
            Self::OpenConfirm { connection_id } => {
                // TODO: Add commitment proofs
                let msg = RawMsgConnectionOpenConfirm {
                    connection_id: connection_id.to_owned(),
                    proof_ack: vec![],
                    proof_height: None,
                    signer: "".to_owned(),
                };

                print_json(msg)?;
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, Subcommand)]
enum MsgKind {
    #[command(subcommand)]
    Client(ClientMsg),
    #[command(subcommand)]
    Connection(ConnectionMsg),
}

#[derive(Debug, Parser)]
pub(crate) struct Args {
    /// Endpoint to send a request to
    #[arg(long, default_value = "http://127.0.0.1:8899")]
    endpoint: String,

    /// Message kind to generate
    #[command(subcommand)]
    kind: MsgKind,
}

pub(crate) async fn run(Args { endpoint, kind }: Args) -> anyhow::Result<()> {
    let rpc_client = RpcClient::new(endpoint);

    match kind {
        MsgKind::Client(msg) => {
            msg.generate(&rpc_client).await?;
        }
        MsgKind::Connection(msg) => {
            msg.generate(&rpc_client).await?;
        }
    }

    Ok(())
}
