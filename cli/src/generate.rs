use {
    anyhow::anyhow,
    clap::{Parser, Subcommand},
    eclipse_ibc_state::{IbcAccountData, IbcState, IbcStore},
    ibc::core::ics24_host::path::{ClientStatePath, ConnectionPath},
    ibc_proto::ibc::core::connection::v1::{
        MsgConnectionOpenAck as RawMsgConnectionOpenAck,
        MsgConnectionOpenConfirm as RawMsgConnectionOpenConfirm,
        MsgConnectionOpenInit as RawMsgConnectionOpenInit,
        MsgConnectionOpenTry as RawMsgConnectionOpenTry,
    },
    solana_client::nonblocking::rpc_client::RpcClient,
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

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Subcommand)]
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
    async fn run(&self, rpc_client: &RpcClient) -> anyhow::Result<()> {
        match self {
            Self::OpenInit { client_id } => {
                let msg = RawMsgConnectionOpenInit {
                    client_id: client_id.to_owned(),
                    counterparty: None,
                    version: None,
                    delay_period: DELAY_PERIOD_NANOS,
                    signer: "".to_owned(),
                };

                let json_str = colored_json::to_colored_json_auto(&serde_json::to_value(msg)?)?;
                println!("{json_str}");

                Ok(())
            }
            Self::OpenTry { client_id } => {
                let ibc_store = get_ibc_store(rpc_client).await?;
                let ibc_state = get_ibc_state(&ibc_store)?;

                let client_state = ibc_state.get_raw(&ClientStatePath::new(&client_id.parse()?))?;

                // TODO: Add commitment proofs
                #[allow(deprecated)]
                let msg = RawMsgConnectionOpenTry {
                    client_id: client_id.to_owned(),
                    previous_connection_id: "".to_owned(),
                    client_state,
                    counterparty: None,
                    delay_period: DELAY_PERIOD_NANOS,
                    counterparty_versions: vec![],
                    proof_height: None,
                    proof_init: vec![],
                    proof_client: vec![],
                    proof_consensus: vec![],
                    consensus_height: None,
                    signer: "".to_owned(),
                };

                let json_str = colored_json::to_colored_json_auto(&serde_json::to_value(msg)?)?;
                println!("{json_str}");

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
                    anyhow!("Counterparty does nto exist for client ID: {client_id}")
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

                let json_str = colored_json::to_colored_json_auto(&serde_json::to_value(msg)?)?;
                println!("{json_str}");

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

                let json_str = colored_json::to_colored_json_auto(&serde_json::to_value(msg)?)?;
                println!("{json_str}");

                Ok(())
            }
        }
    }
}

#[derive(Debug, Subcommand)]
enum MsgKind {
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
        MsgKind::Connection(msg) => {
            msg.run(&rpc_client).await?;
        }
    }

    Ok(())
}
