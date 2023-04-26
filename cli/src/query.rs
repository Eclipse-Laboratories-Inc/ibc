use {
    anyhow::anyhow,
    clap::{Parser, Subcommand},
    eclipse_ibc_known_path::KnownPath,
    eclipse_ibc_known_proto::KnownProto,
    eclipse_ibc_light_client::{eclipse_chain, EclipseConsensusState},
    eclipse_ibc_state::{
        decode_client_state, decode_consensus_state,
        internal_path::{
            AllModulesPath, ClientUpdateHeightPath, ClientUpdateTimePath, ConsensusHeightsPath,
        },
        IbcAccountData, IbcState,
    },
    ibc::core::{
        ics02_client::height::Height,
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
    serde::Serialize,
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::hash::Hash,
    std::{
        collections::HashMap,
        io::{self, Write as _},
        sync::Arc,
    },
    tendermint::time::Time as TendermintTime,
};

fn print_json<T>(msg: T) -> anyhow::Result<()>
where
    T: Serialize,
{
    let json_str = colored_json::to_colored_json_auto(&serde_json::to_value(msg)?)?;
    writeln!(io::stdout(), "{json_str}")?;
    Ok(())
}

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
            Self::ClientState { client_id } => get_json_with_decode(
                ibc_state,
                &ClientStatePath::new(client_id),
                decode_client_state,
            ),
            Self::ConsensusState { client_id, height } => get_json_with_decode(
                ibc_state,
                &ClientConsensusStatePath::new(client_id, height),
                decode_consensus_state,
            ),
            Self::Connection { connection_id } => {
                get_json(ibc_state, &ConnectionPath::new(connection_id))
            }
            Self::ClientConnections { client_id } => {
                get_json(ibc_state, &ClientConnectionPath::new(client_id))
            }
            Self::Channel {
                port_id,
                channel_id,
            } => get_json(ibc_state, &ChannelEndPath::new(port_id, channel_id)),
            Self::NextSequenceSend {
                port_id,
                channel_id,
            } => get_json(ibc_state, &SeqSendPath::new(port_id, channel_id)),
            Self::NextSequenceRecv {
                port_id,
                channel_id,
            } => get_json(ibc_state, &SeqRecvPath::new(port_id, channel_id)),
            Self::NextSequenceAck {
                port_id,
                channel_id,
            } => get_json(ibc_state, &SeqAckPath::new(port_id, channel_id)),
            Self::PacketCommitment {
                port_id,
                channel_id,
                sequence,
            } => get_json(
                ibc_state,
                &CommitmentPath::new(port_id, channel_id, *sequence),
            ),
            Self::PacketReceipt {
                port_id,
                channel_id,
                sequence,
            } => get_json(ibc_state, &ReceiptPath::new(port_id, channel_id, *sequence)),
            Self::PacketAcknowledgement {
                port_id,
                channel_id,
                sequence,
            } => get_json(ibc_state, &AckPath::new(port_id, channel_id, *sequence)),
            Self::Port { port_id } => get_json(ibc_state, &PortPath(port_id.clone())),
            Self::ClientUpdateTime { client_id, height } => {
                get_json(ibc_state, &ClientUpdateTimePath(client_id.clone(), *height))
            }
            Self::ClientUpdateHeight { client_id, height } => get_json(
                ibc_state,
                &ClientUpdateHeightPath(client_id.clone(), *height),
            ),
            Self::ConsensusHeights { client_id } => {
                get_json(ibc_state, &ConsensusHeightsPath(client_id.clone()))
            }
            Self::AllModules => get_json(ibc_state, &AllModulesPath),
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
        writeln!(io::stdout(), "{json_str}")?;

        Ok(())
    }
}

fn get_json_with_decode<K, T, E>(
    ibc_state: &IbcState<'_>,
    key: &K,
    decode: impl FnOnce(<K::Value as KnownProto>::Raw) -> Result<T, E>,
) -> anyhow::Result<String>
where
    K: KnownPath,
    T: Serialize,
    anyhow::Error: From<E>,
{
    let raw = ibc_state
        .get_raw(key)?
        .ok_or_else(|| anyhow!("No value found for key: {key}"))?;
    let decoded_raw = decode(raw)?;
    Ok(colored_json::to_colored_json_auto(&serde_json::to_value(
        &decoded_raw,
    )?)?)
}

fn get_json<K>(ibc_state: &IbcState<'_>, key: &K) -> anyhow::Result<String>
where
    K: KnownPath,
    <K::Value as KnownProto>::Raw: Serialize,
{
    get_json_with_decode(ibc_state, key, anyhow::Ok)
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
                writeln!(io::stdout(), "{height}")?;

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

                print_json(consensus_state)?;
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

                print_json(ibc_metadata)?;
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

                print_json(ibc_state_map)?;
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
