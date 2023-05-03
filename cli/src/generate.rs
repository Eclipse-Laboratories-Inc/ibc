use {
    crate::chain_state,
    anyhow::{anyhow, bail},
    clap::{Parser, Subcommand},
    eclipse_ibc_known_proto::KnownAnyProto,
    eclipse_ibc_light_client::eclipse_chain,
    eclipse_ibc_state::{internal_path::ConsensusHeightsPath, IbcAccountData, IbcState, IbcStore},
    ibc::core::{
        ics02_client::height::Height,
        ics03_connection::version::{get_compatible_versions, Version as ConnectionVersion},
        ics24_host::path::{
            ChannelEndPath, ClientConsensusStatePath, ClientStatePath, ConnectionPath,
        },
    },
    ibc_proto::{
        ibc::core::{
            channel::v1::{
                Channel as RawChannel, Counterparty as RawChannelCounterparty,
                MsgChannelOpenAck as RawMsgChannelOpenAck,
                MsgChannelOpenConfirm as RawMsgChannelOpenConfirm,
                MsgChannelOpenInit as RawMsgChannelOpenInit,
                MsgChannelOpenTry as RawMsgChannelOpenTry, Order as RawOrder, State as RawState,
            },
            client::v1::{
                MsgCreateClient as RawMsgCreateClient, MsgUpdateClient as RawMsgUpdateClient,
                MsgUpgradeClient as RawMsgUpgradeClient,
            },
            commitment::v1::{MerklePrefix as RawMerklePrefix, MerkleProof as RawMerkleProof},
            connection::v1::{
                Counterparty as RawConnectionCounterparty,
                MsgConnectionOpenAck as RawMsgConnectionOpenAck,
                MsgConnectionOpenConfirm as RawMsgConnectionOpenConfirm,
                MsgConnectionOpenInit as RawMsgConnectionOpenInit,
                MsgConnectionOpenTry as RawMsgConnectionOpenTry,
            },
        },
        ics23::CommitmentProof as IbcRawCommitmentProof,
    },
    ics23::{commitment_proof, CommitmentProof, ExistenceProof},
    log::info,
    prost::Message as _,
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

fn existence_proof_to_merkle_proof(existence_proof: ExistenceProof) -> RawMerkleProof {
    let commitment_proof = CommitmentProof {
        proof: Some(commitment_proof::Proof::Exist(existence_proof)),
    };
    let ibc_commitment_proof = IbcRawCommitmentProof::decode(&*commitment_proof.encode_to_vec())
        .expect("CommitmentProof should be the same between ics23 and ibc-proto");

    RawMerkleProof {
        proofs: vec![ibc_commitment_proof],
    }
}

fn get_latest_consensus_height(ibc_state: &IbcState, client_id: &str) -> anyhow::Result<Height> {
    Ok(*ibc_state
        .get(&ConsensusHeightsPath(client_id.parse()?))?
        .ok_or_else(|| anyhow!("Consensus heights not found for client ID {client_id}"))?
        .heights
        .last()
        .ok_or_else(|| anyhow!("No consensus heights found for client ID {client_id}"))?)
}

async fn get_and_verify_consensus_height_on_cpty(
    ibc_store: &IbcStore,
    cpty_rpc_client: &RpcClient,
    client_id_on_cpty: &str,
) -> anyhow::Result<Height> {
    let ibc_latest_version = ibc_store
        .read()?
        .latest_version()
        .ok_or_else(|| anyhow!("No IBC state versions found"))?;
    let ibc_latest_height = eclipse_chain::height_of_slot(ibc_latest_version)?;

    let cpty_ibc_store = get_ibc_store(cpty_rpc_client).await?;
    let cpty_ibc_state = get_ibc_state(&cpty_ibc_store)?;

    let consensus_height_on_cpty = get_latest_consensus_height(&cpty_ibc_state, client_id_on_cpty)?;

    if consensus_height_on_cpty < ibc_latest_height {
        bail!(
            "Height of chain (client ID {client_id_on_cpty}) on cpty chain is not recent enough; \
               {consensus_height_on_cpty} < {ibc_latest_height}"
        );
    }

    Ok(consensus_height_on_cpty)
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
        client_id_on_a: String,
        client_id_on_b: String,
    },
    OpenTry {
        client_id_on_b: String,
        client_id_on_a: String,
        connection_id_on_a: String,
    },
    OpenAck {
        client_id_on_a: String,
        connection_id_on_a: String,
        client_id_on_b: String,
        connection_id_on_b: String,
    },
    OpenConfirm {
        client_id_on_b: String,
        connection_id_on_b: String,
        connection_id_on_a: String,
    },
}

impl ConnectionMsg {
    async fn generate(
        &self,
        rpc_client: &RpcClient,
        cpty_rpc_client: &RpcClient,
    ) -> anyhow::Result<()> {
        match self {
            Self::OpenInit {
                client_id_on_a,
                client_id_on_b,
            } => {
                let counterparty = RawConnectionCounterparty {
                    client_id: client_id_on_b.clone(),
                    connection_id: "".to_owned(),
                    prefix: Some(RawMerklePrefix {
                        key_prefix: eclipse_chain::COMMITMENT_PREFIX.to_vec(),
                    }),
                };

                let msg = RawMsgConnectionOpenInit {
                    client_id: client_id_on_a.clone(),
                    counterparty: Some(counterparty),
                    version: Some(ConnectionVersion::default().into()),
                    delay_period: DELAY_PERIOD_NANOS,
                    signer: "".to_owned(),
                };

                print_json(msg)?;
                Ok(())
            }
            Self::OpenTry {
                client_id_on_b,
                client_id_on_a,
                connection_id_on_a,
            } => {
                let counterparty = RawConnectionCounterparty {
                    client_id: client_id_on_a.clone(),
                    connection_id: connection_id_on_a.clone(),
                    prefix: Some(RawMerklePrefix {
                        key_prefix: eclipse_chain::COMMITMENT_PREFIX.to_vec(),
                    }),
                };

                let ibc_store = get_ibc_store(rpc_client).await?;
                let ibc_state = get_ibc_state(&ibc_store)?;

                let client_state =
                    ibc_state.get_raw(&ClientStatePath::new(&client_id_on_a.parse()?))?;
                let consensus_height_of_b_on_a =
                    get_latest_consensus_height(&ibc_state, client_id_on_a)?;

                let proof_init = existence_proof_to_merkle_proof(
                    ibc_state.get_proof(&ConnectionPath::new(&connection_id_on_a.parse()?))?,
                );
                let proof_client = existence_proof_to_merkle_proof(
                    ibc_state.get_proof(&ClientStatePath::new(&client_id_on_a.parse()?))?,
                );
                let proof_consensus = existence_proof_to_merkle_proof(ibc_state.get_proof(
                    &ClientConsensusStatePath::new(
                        &client_id_on_a.parse()?,
                        &consensus_height_of_b_on_a,
                    ),
                )?);

                let consensus_height_of_a_on_b = get_and_verify_consensus_height_on_cpty(
                    &ibc_store,
                    cpty_rpc_client,
                    client_id_on_b,
                )
                .await?;

                #[allow(deprecated)]
                let msg = RawMsgConnectionOpenTry {
                    client_id: client_id_on_b.clone(),
                    previous_connection_id: "".to_owned(),
                    client_state,
                    counterparty: Some(counterparty),
                    delay_period: DELAY_PERIOD_NANOS,
                    counterparty_versions: get_compatible_versions()
                        .into_iter()
                        .map(ConnectionVersion::into)
                        .collect(),
                    proof_height: Some(consensus_height_of_a_on_b.into()),
                    proof_init: proof_init.encode_to_vec(),
                    proof_client: proof_client.encode_to_vec(),
                    proof_consensus: proof_consensus.encode_to_vec(),
                    consensus_height: Some(consensus_height_of_b_on_a.into()),
                    signer: "".to_owned(),
                };

                print_json(msg)?;
                Ok(())
            }
            Self::OpenAck {
                client_id_on_a,
                connection_id_on_a,
                client_id_on_b,
                connection_id_on_b,
            } => {
                let ibc_store = get_ibc_store(rpc_client).await?;
                let ibc_state = get_ibc_state(&ibc_store)?;

                let client_state =
                    ibc_state.get_raw(&ClientStatePath::new(&client_id_on_b.parse()?))?;
                let consensus_height_of_a_on_b =
                    get_latest_consensus_height(&ibc_state, client_id_on_b)?;

                let proof_try = existence_proof_to_merkle_proof(
                    ibc_state.get_proof(&ConnectionPath::new(&connection_id_on_b.parse()?))?,
                );
                let proof_client = existence_proof_to_merkle_proof(
                    ibc_state.get_proof(&ClientStatePath::new(&client_id_on_b.parse()?))?,
                );
                let proof_consensus = existence_proof_to_merkle_proof(ibc_state.get_proof(
                    &ClientConsensusStatePath::new(
                        &client_id_on_b.parse()?,
                        &consensus_height_of_a_on_b,
                    ),
                )?);

                let consensus_height_of_b_on_a = get_and_verify_consensus_height_on_cpty(
                    &ibc_store,
                    cpty_rpc_client,
                    client_id_on_a,
                )
                .await?;

                let msg = RawMsgConnectionOpenAck {
                    connection_id: connection_id_on_a.clone(),
                    counterparty_connection_id: connection_id_on_b.clone(),
                    version: Some(ConnectionVersion::default().into()),
                    client_state,
                    proof_height: Some(consensus_height_of_b_on_a.into()),
                    proof_try: proof_try.encode_to_vec(),
                    proof_client: proof_client.encode_to_vec(),
                    proof_consensus: proof_consensus.encode_to_vec(),
                    consensus_height: Some(consensus_height_of_a_on_b.into()),
                    signer: "".to_owned(),
                };

                print_json(msg)?;
                Ok(())
            }
            Self::OpenConfirm {
                client_id_on_b,
                connection_id_on_b,
                connection_id_on_a,
            } => {
                let ibc_store = get_ibc_store(rpc_client).await?;
                let ibc_state = get_ibc_state(&ibc_store)?;

                let proof_ack = existence_proof_to_merkle_proof(
                    ibc_state.get_proof(&ConnectionPath::new(&connection_id_on_a.parse()?))?,
                );

                let consensus_height_of_a_on_b = get_and_verify_consensus_height_on_cpty(
                    &ibc_store,
                    cpty_rpc_client,
                    client_id_on_b,
                )
                .await?;

                let msg = RawMsgConnectionOpenConfirm {
                    connection_id: connection_id_on_b.clone(),
                    proof_ack: proof_ack.encode_to_vec(),
                    proof_height: Some(consensus_height_of_a_on_b.into()),
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
enum ChannelMsg {
    OpenInit {
        connection_id_on_a: String,
        port_id_on_a: String,
        port_id_on_b: String,
    },
    OpenTry {
        client_id_on_b: String,
        connection_id_on_b: String,
        port_id_on_b: String,
        port_id_on_a: String,
        channel_id_on_a: String,
    },
    OpenAck {
        client_id_on_a: String,
        port_id_on_a: String,
        channel_id_on_a: String,
        port_id_on_b: String,
        channel_id_on_b: String,
    },
    OpenConfirm {
        client_id_on_b: String,
        port_id_on_b: String,
        channel_id_on_b: String,
        port_id_on_a: String,
        channel_id_on_a: String,
    },
}

impl ChannelMsg {
    async fn generate(
        &self,
        rpc_client: &RpcClient,
        cpty_rpc_client: &RpcClient,
    ) -> anyhow::Result<()> {
        match self {
            Self::OpenInit {
                connection_id_on_a,
                port_id_on_a,
                port_id_on_b,
            } => {
                let counterparty = RawChannelCounterparty {
                    port_id: port_id_on_b.clone(),
                    channel_id: "".to_owned(),
                };

                let channel = RawChannel {
                    state: RawState::Init.into(),
                    ordering: RawOrder::Ordered.into(),
                    counterparty: Some(counterparty),
                    connection_hops: vec![connection_id_on_a.clone()],
                    version: "".to_owned(),
                };

                let msg = RawMsgChannelOpenInit {
                    port_id: port_id_on_a.clone(),
                    channel: Some(channel),
                    signer: "".to_owned(),
                };

                print_json(msg)?;
                Ok(())
            }
            Self::OpenTry {
                client_id_on_b,
                connection_id_on_b,
                port_id_on_b,
                port_id_on_a,
                channel_id_on_a,
            } => {
                let counterparty = RawChannelCounterparty {
                    port_id: port_id_on_a.clone(),
                    channel_id: channel_id_on_a.clone(),
                };

                let channel = RawChannel {
                    state: RawState::Tryopen.into(),
                    ordering: RawOrder::Ordered.into(),
                    counterparty: Some(counterparty),
                    connection_hops: vec![connection_id_on_b.clone()],
                    version: "".to_owned(),
                };

                let ibc_store = get_ibc_store(rpc_client).await?;
                let ibc_state = get_ibc_state(&ibc_store)?;

                let proof_init = existence_proof_to_merkle_proof(ibc_state.get_proof(
                    &ChannelEndPath::new(&port_id_on_a.parse()?, &channel_id_on_a.parse()?),
                )?);

                let consensus_height_of_a_on_b = get_and_verify_consensus_height_on_cpty(
                    &ibc_store,
                    cpty_rpc_client,
                    client_id_on_b,
                )
                .await?;

                #[allow(deprecated)]
                let msg = RawMsgChannelOpenTry {
                    port_id: port_id_on_b.clone(),
                    previous_channel_id: "".to_owned(),
                    channel: Some(channel),
                    counterparty_version: "".to_owned(),
                    proof_init: proof_init.encode_to_vec(),
                    proof_height: Some(consensus_height_of_a_on_b.into()),
                    signer: "".to_owned(),
                };

                print_json(msg)?;
                Ok(())
            }
            Self::OpenAck {
                client_id_on_a,
                port_id_on_a,
                channel_id_on_a,
                port_id_on_b,
                channel_id_on_b,
            } => {
                let ibc_store = get_ibc_store(rpc_client).await?;
                let ibc_state = get_ibc_state(&ibc_store)?;

                let proof_try = existence_proof_to_merkle_proof(ibc_state.get_proof(
                    &ChannelEndPath::new(&port_id_on_b.parse()?, &channel_id_on_b.parse()?),
                )?);

                let consensus_height_of_b_on_a = get_and_verify_consensus_height_on_cpty(
                    &ibc_store,
                    cpty_rpc_client,
                    client_id_on_a,
                )
                .await?;

                let msg = RawMsgChannelOpenAck {
                    port_id: port_id_on_a.clone(),
                    channel_id: channel_id_on_a.clone(),
                    counterparty_channel_id: channel_id_on_b.clone(),
                    counterparty_version: "".to_owned(),
                    proof_try: proof_try.encode_to_vec(),
                    proof_height: Some(consensus_height_of_b_on_a.into()),
                    signer: "".to_owned(),
                };

                print_json(msg)?;
                Ok(())
            }
            Self::OpenConfirm {
                client_id_on_b,
                port_id_on_b,
                channel_id_on_b,
                port_id_on_a,
                channel_id_on_a,
            } => {
                let ibc_store = get_ibc_store(rpc_client).await?;
                let ibc_state = get_ibc_state(&ibc_store)?;

                let proof_ack = existence_proof_to_merkle_proof(ibc_state.get_proof(
                    &ChannelEndPath::new(&port_id_on_a.parse()?, &channel_id_on_a.parse()?),
                )?);

                let consensus_height_of_a_on_b = get_and_verify_consensus_height_on_cpty(
                    &ibc_store,
                    cpty_rpc_client,
                    client_id_on_b,
                )
                .await?;

                let msg = RawMsgChannelOpenConfirm {
                    port_id: port_id_on_b.clone(),
                    channel_id: channel_id_on_b.clone(),
                    proof_ack: proof_ack.encode_to_vec(),
                    proof_height: Some(consensus_height_of_a_on_b.into()),
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
    #[command(subcommand)]
    Channel(ChannelMsg),
}

#[derive(Debug, Parser)]
pub(crate) struct Args {
    /// Endpoint to send a request to
    #[arg(long, default_value = "http://127.0.0.1:8899")]
    endpoint: String,

    /// Counterparty endpoint to send a request to
    #[arg(long)]
    cpty_endpoint: Option<String>,

    /// Message kind to generate
    #[command(subcommand)]
    kind: MsgKind,
}

pub(crate) async fn run(
    Args {
        endpoint,
        cpty_endpoint,
        kind,
    }: Args,
) -> anyhow::Result<()> {
    let rpc_client = RpcClient::new(endpoint);

    info!("Generating IBC tx: {kind:?}");
    match kind {
        MsgKind::Client(msg) => {
            msg.generate(&rpc_client).await?;
        }
        MsgKind::Connection(msg) => {
            let cpty_endpoint =
                cpty_endpoint.ok_or_else(|| anyhow!("Must specify counterparty endpoint"))?;
            let cpty_rpc_client = RpcClient::new(cpty_endpoint);
            msg.generate(&rpc_client, &cpty_rpc_client).await?;
        }
        MsgKind::Channel(msg) => {
            let cpty_endpoint =
                cpty_endpoint.ok_or_else(|| anyhow!("Must specify counterparty endpoint"))?;
            let cpty_rpc_client = RpcClient::new(cpty_endpoint);
            msg.generate(&rpc_client, &cpty_rpc_client).await?;
        }
    }

    Ok(())
}
