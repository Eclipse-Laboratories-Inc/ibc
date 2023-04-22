use {
    anyhow::anyhow,
    clap::{Parser, Subcommand},
    eclipse_ibc_known_path::KnownPath,
    eclipse_ibc_light_client::{eclipse_chain, EclipseConsensusState},
    eclipse_ibc_proto::eclipse::ibc::client::v1::{
        AllModuleIds as RawAllModuleIds, ClientConnections as RawClientConnections,
        ConsensusHeights as RawConsensusHeights,
    },
    eclipse_ibc_state::{
        decode_client_state, decode_consensus_state,
        internal_path::{
            AllModulesPath, ClientUpdateHeightPath, ClientUpdateTimePath, ConsensusHeightsPath,
        },
        IbcAccountData, IbcState,
    },
    ibc::core::{
        ics02_client::{
            client_state::ClientState, consensus_state::ConsensusState, height::Height,
        },
        ics04_channel::packet::Sequence,
        ics23_commitment::commitment::CommitmentRoot,
        ics24_host::{
            identifier::{ChannelId, ClientId, ConnectionId, PortId},
            path::{
                AckPath, ChannelEndPath, ClientConnectionPath, ClientConsensusStatePath,
                ClientStatePath, CommitmentPath, ConnectionPath, PortPath, ReceiptPath, SeqAckPath,
                SeqRecvPath, SeqSendPath,
            },
        },
    },
    ibc_proto::{
        google::protobuf,
        ibc::core::{
            channel::v1::Channel as RawChannel, client::v1::Height as RawHeight,
            connection::v1::ConnectionEnd as RawConnectionEnd,
        },
    },
    serde::Serialize,
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::hash::Hash,
    std::{collections::HashMap, sync::Arc},
    tendermint::time::Time as TendermintTime,
};

#[derive(Clone, Debug, Subcommand)]
enum StateKind {
    #[command(flatten)]
    Merkle(MerkleStateKind),

    #[command(flatten)]
    Chain(ChainStateKind),
}

#[derive(Clone, Debug, Subcommand)]
enum MerkleStateKind {
    ClientState {
        client_id: ClientId,
    },
    ConsensusState {
        client_id: ClientId,
        height: Height,
    },
    Connection {
        connection_id: ConnectionId,
    },
    ClientConnections {
        client_id: ClientId,
    },
    Channel {
        port_id: PortId,
        channel_id: ChannelId,
    },
    NextSequenceSend {
        port_id: PortId,
        channel_id: ChannelId,
    },
    NextSequenceRecv {
        port_id: PortId,
        channel_id: ChannelId,
    },
    NextSequenceAck {
        port_id: PortId,
        channel_id: ChannelId,
    },
    PacketCommitment {
        port_id: PortId,
        channel_id: ChannelId,
        sequence: Sequence,
    },
    PacketReceipt {
        port_id: PortId,
        channel_id: ChannelId,
        sequence: Sequence,
    },
    PacketAcknowledgement {
        port_id: PortId,
        channel_id: ChannelId,
        sequence: Sequence,
    },
    Port {
        port_id: PortId,
    },
    ClientUpdateTime {
        client_id: ClientId,
        height: Height,
    },
    ClientUpdateHeight {
        client_id: ClientId,
        height: Height,
    },
    ConsensusHeights {
        client_id: ClientId,
    },
    AllModules,
}

impl MerkleStateKind {
    fn get_json_str(&self, ibc_state: &IbcState<'_>) -> anyhow::Result<String> {
        match self {
            Self::ClientState { client_id } => get_json_from_any::<_, Box<dyn ClientState>, _, _>(
                ibc_state,
                &ClientStatePath::new(client_id),
                decode_client_state,
            ),
            Self::ConsensusState { client_id, height } => {
                get_json_from_any::<_, Box<dyn ConsensusState>, _, _>(
                    ibc_state,
                    &ClientConsensusStatePath::new(client_id, height),
                    decode_consensus_state,
                )
            }
            Self::Connection { connection_id } => {
                get_json::<_, RawConnectionEnd>(ibc_state, &ConnectionPath::new(connection_id))
            }
            Self::ClientConnections { client_id } => get_json::<_, RawClientConnections>(
                ibc_state,
                &ClientConnectionPath::new(client_id),
            ),
            Self::Channel {
                port_id,
                channel_id,
            } => get_json::<_, RawChannel>(ibc_state, &ChannelEndPath::new(port_id, channel_id)),
            Self::NextSequenceSend {
                port_id,
                channel_id,
            } => get_json::<_, u64>(ibc_state, &SeqSendPath::new(port_id, channel_id)),
            Self::NextSequenceRecv {
                port_id,
                channel_id,
            } => get_json::<_, u64>(ibc_state, &SeqRecvPath::new(port_id, channel_id)),
            Self::NextSequenceAck {
                port_id,
                channel_id,
            } => get_json::<_, u64>(ibc_state, &SeqAckPath::new(port_id, channel_id)),
            Self::PacketCommitment {
                port_id,
                channel_id,
                sequence,
            } => get_json::<_, Vec<u8>>(
                ibc_state,
                &CommitmentPath::new(port_id, channel_id, *sequence),
            ),
            Self::PacketReceipt {
                port_id,
                channel_id,
                sequence,
            } => {
                get_json::<_, Vec<u8>>(ibc_state, &ReceiptPath::new(port_id, channel_id, *sequence))
            }
            Self::PacketAcknowledgement {
                port_id,
                channel_id,
                sequence,
            } => get_json::<_, Vec<u8>>(ibc_state, &AckPath::new(port_id, channel_id, *sequence)),
            Self::Port { port_id } => get_json::<_, String>(ibc_state, &PortPath(port_id.clone())),
            Self::ClientUpdateTime { client_id, height } => {
                get_json::<_, u64>(ibc_state, &ClientUpdateTimePath(client_id.clone(), *height))
            }
            Self::ClientUpdateHeight { client_id, height } => get_json::<_, RawHeight>(
                ibc_state,
                &ClientUpdateHeightPath(client_id.clone(), *height),
            ),
            Self::ConsensusHeights { client_id } => get_json::<_, RawConsensusHeights>(
                ibc_state,
                &ConsensusHeightsPath(client_id.clone()),
            ),
            Self::AllModules => get_json::<_, RawAllModuleIds>(ibc_state, &AllModulesPath),
        }
    }

    async fn run(self, rpc_client: &RpcClient) -> anyhow::Result<()> {
        let raw_account_data = rpc_client
            .get_account_data(&eclipse_ibc_program::STORAGE_KEY)
            .await?;

        let IbcAccountData {
            store: ibc_store, ..
        } = bincode::deserialize(&raw_account_data)?;

        let latest_version = ibc_store
            .read()?
            .latest_version()
            .ok_or_else(|| anyhow!("IBC store is missing latest version"))?;

        let ibc_state = IbcState::new(&ibc_store, latest_version);

        let json_str = self.get_json_str(&ibc_state)?;
        println!("{json_str}");

        Ok(())
    }
}

fn get_json<K, T>(ibc_state: &IbcState<'_>, key: &K) -> anyhow::Result<String>
where
    K: KnownPath,
    T: Default + prost::Message + Serialize,
{
    let raw = ibc_state
        .get_raw::<K, T>(key)?
        .ok_or_else(|| anyhow!("No value found for key: {key}"))?;
    Ok(colored_json::to_colored_json_auto(&serde_json::to_value(
        &raw,
    )?)?)
}

fn get_json_from_any<K, T, F, E>(
    ibc_state: &IbcState<'_>,
    key: &K,
    decode_any: F,
) -> anyhow::Result<String>
where
    K: KnownPath,
    T: Serialize,
    F: FnOnce(protobuf::Any) -> Result<T, E>,
    E: std::error::Error + Send + Sync + 'static,
{
    let raw_any = ibc_state
        .get_raw::<K, protobuf::Any>(key)?
        .ok_or_else(|| anyhow!("No value found for key: {key}"))?;
    let raw = decode_any(raw_any)?;
    Ok(colored_json::to_colored_json_auto(&serde_json::to_value(
        &raw,
    )?)?)
}

#[derive(Clone, Debug, Subcommand)]
enum ChainStateKind {
    HostHeight,
    HostConsensusState { height: Height },
    IbcMetadata,
    IbcState,
}

impl ChainStateKind {
    async fn run(self, rpc_client: &RpcClient) -> anyhow::Result<()> {
        match self {
            Self::HostHeight => {
                let slot = rpc_client.get_slot().await?;
                let height = eclipse_chain::height_of_slot(slot)?;
                println!("{height}");

                Ok(())
            }
            Self::HostConsensusState { height } => {
                let slot = eclipse_chain::slot_of_height(height)?;
                let block = rpc_client.get_block(slot).await?;
                let commitment_root =
                    CommitmentRoot::from_bytes(block.blockhash.parse::<Hash>()?.as_ref());
                let timestamp = TendermintTime::from_unix_timestamp(
                    block
                        .block_time
                        .ok_or_else(|| anyhow!("Block timestamp should not be missing"))?,
                    0,
                )
                .expect("Block time should be valid");
                let consensus_state = EclipseConsensusState {
                    commitment_root,
                    timestamp,
                };
                let json_str =
                    colored_json::to_colored_json_auto(&serde_json::to_value(consensus_state)?)?;
                println!("{json_str}");

                Ok(())
            }
            Self::IbcMetadata => {
                let raw_account_data = rpc_client
                    .get_account_data(&eclipse_ibc_program::STORAGE_KEY)
                    .await?;

                let IbcAccountData {
                    metadata: ibc_metadata,
                    ..
                } = bincode::deserialize(&raw_account_data)?;

                let json_str =
                    colored_json::to_colored_json_auto(&serde_json::to_value(ibc_metadata)?)?;
                println!("{json_str}");

                Ok(())
            }
            Self::IbcState => {
                let raw_account_data = rpc_client
                    .get_account_data(&eclipse_ibc_program::STORAGE_KEY)
                    .await?;

                let IbcAccountData {
                    store: ibc_store, ..
                } = bincode::deserialize(&raw_account_data)?;

                let latest_version = ibc_store
                    .read()?
                    .latest_version()
                    .ok_or_else(|| anyhow!("IBC store is missing latest version"))?;

                let ibc_jmt_iter = jmt::JellyfishMerkleIterator::new_by_index(
                    Arc::new(ibc_store),
                    latest_version,
                    0,
                )?;

                let ibc_state_map = ibc_jmt_iter
                    .inspect(|result| {
                        if let Err(err) = result {
                            eprintln!("{err}");
                        }
                    })
                    .filter_map(Result::ok)
                    .map(|(key_hash, value)| (hex::encode(key_hash.0), hex::encode(value)))
                    .collect::<HashMap<_, _>>();

                let json_str =
                    colored_json::to_colored_json_auto(&serde_json::to_value(ibc_state_map)?)?;
                println!("{json_str}");

                Ok(())
            }
        }
    }
}

#[derive(Debug, Parser)]
pub(crate) struct Args {
    /// Endpoint to send a request to
    #[arg(long, default_value = "http://127.0.0.1:8899")]
    endpoint: String,

    /// State kind to query
    #[command(subcommand)]
    kind: StateKind,
}

pub(crate) async fn run(Args { endpoint, kind }: Args) -> anyhow::Result<()> {
    let rpc_client = RpcClient::new(endpoint);

    match kind {
        StateKind::Merkle(merkle_kind) => merkle_kind.run(&rpc_client).await?,
        StateKind::Chain(chain_kind) => chain_kind.run(&rpc_client).await?,
    }

    Ok(())
}
