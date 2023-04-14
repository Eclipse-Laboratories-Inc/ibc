use {
    anyhow::anyhow,
    clap::{Parser, Subcommand},
    eclipse_ibc_program::STORAGE_KEY,
    eclipse_ibc_proto::eclipse::ibc::client::v1::{
        AllModuleIds as RawAllModuleIds, ClientConnections as RawClientConnections,
        ConsensusHeights as RawConsensusHeights,
    },
    eclipse_ibc_state::{
        internal_path::{
            AllModulesPath, ClientUpdateHeightPath, ClientUpdateTimePath, ConsensusHeightsPath,
        },
        IbcAccountData, IbcState,
    },
    ibc::core::{
        ics02_client::height::Height,
        ics04_channel::packet::Sequence,
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
    solana_sdk::pubkey::Pubkey,
};

#[derive(Clone, Debug, Subcommand)]
enum StateKind {
    ClientState {
        client_id: ClientId,
    },
    ConsensusState {
        client_id: ClientId,
        epoch: u64,
        height: u64,
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

impl StateKind {
    fn into_path(self) -> String {
        match self {
            Self::ClientState { client_id } => ClientStatePath(client_id).to_string(),
            Self::ConsensusState {
                client_id,
                epoch,
                height,
            } => ClientConsensusStatePath {
                client_id,
                epoch,
                height,
            }
            .to_string(),
            Self::Connection { connection_id } => ConnectionPath(connection_id).to_string(),
            Self::ClientConnections { client_id } => ClientConnectionPath(client_id).to_string(),
            Self::Channel {
                port_id,
                channel_id,
            } => ChannelEndPath(port_id, channel_id).to_string(),
            Self::NextSequenceSend {
                port_id,
                channel_id,
            } => SeqSendPath(port_id, channel_id).to_string(),
            Self::NextSequenceRecv {
                port_id,
                channel_id,
            } => SeqRecvPath(port_id, channel_id).to_string(),
            Self::NextSequenceAck {
                port_id,
                channel_id,
            } => SeqAckPath(port_id, channel_id).to_string(),
            Self::PacketCommitment {
                port_id,
                channel_id,
                sequence,
            } => CommitmentPath {
                port_id,
                channel_id,
                sequence,
            }
            .to_string(),
            Self::PacketReceipt {
                port_id,
                channel_id,
                sequence,
            } => ReceiptPath {
                port_id,
                channel_id,
                sequence,
            }
            .to_string(),
            Self::PacketAcknowledgement {
                port_id,
                channel_id,
                sequence,
            } => AckPath {
                port_id,
                channel_id,
                sequence,
            }
            .to_string(),
            Self::Port { port_id } => PortPath(port_id).to_string(),
            Self::ClientUpdateTime { client_id, height } => {
                ClientUpdateTimePath(client_id, height).to_string()
            }
            Self::ClientUpdateHeight { client_id, height } => {
                ClientUpdateHeightPath(client_id, height).to_string()
            }
            Self::ConsensusHeights { client_id } => ConsensusHeightsPath(client_id).to_string(),
            Self::AllModules => AllModulesPath.to_string(),
        }
    }

    fn get_json_str(&self, ibc_state: &IbcState<'_>) -> anyhow::Result<String> {
        let path = self.clone().into_path();
        match self {
            Self::ClientState { .. } => get_json::<protobuf::Any>(ibc_state, &path),
            Self::ConsensusState { .. } => get_json::<protobuf::Any>(ibc_state, &path),
            Self::Connection { .. } => get_json::<RawConnectionEnd>(ibc_state, &path),
            Self::ClientConnections { .. } => get_json::<RawClientConnections>(ibc_state, &path),
            Self::Channel { .. } => get_json::<RawChannel>(ibc_state, &path),
            Self::NextSequenceSend { .. } => get_json::<u64>(ibc_state, &path),
            Self::NextSequenceRecv { .. } => get_json::<u64>(ibc_state, &path),
            Self::NextSequenceAck { .. } => get_json::<u64>(ibc_state, &path),
            Self::PacketCommitment { .. } => get_json::<Vec<u8>>(ibc_state, &path),
            Self::PacketReceipt { .. } => get_json::<Vec<u8>>(ibc_state, &path),
            Self::PacketAcknowledgement { .. } => get_json::<Vec<u8>>(ibc_state, &path),
            Self::Port { .. } => get_json::<String>(ibc_state, &path),
            Self::ClientUpdateTime { .. } => get_json::<u64>(ibc_state, &path),
            Self::ClientUpdateHeight { .. } => get_json::<RawHeight>(ibc_state, &path),
            Self::ConsensusHeights { .. } => get_json::<RawConsensusHeights>(ibc_state, &path),
            Self::AllModules => get_json::<RawAllModuleIds>(ibc_state, &path),
        }
    }
}

fn get_json<T>(ibc_state: &IbcState<'_>, key: &str) -> anyhow::Result<String>
where
    T: Default + prost::Message + Serialize,
{
    let raw = ibc_state
        .get_raw::<T>(key)?
        .ok_or_else(|| anyhow!("No value found for key: {key}"))?;
    Ok(serde_json::to_string_pretty(&raw)?)
}

#[derive(Debug, Parser)]
pub(crate) struct Args {
    /// Address of IBC storage account
    #[arg(long, default_value_t = STORAGE_KEY)]
    address: Pubkey,

    /// Endpoint to send a request to
    #[arg(long, default_value = "http://127.0.0.1:8899")]
    endpoint: String,

    /// State kind to query
    #[command(subcommand)]
    kind: StateKind,
}

pub(crate) async fn run(
    Args {
        address,
        endpoint,
        kind,
    }: Args,
) -> anyhow::Result<()> {
    let path = kind.clone().into_path();
    println!("{path}:");

    let rpc_client = RpcClient::new(endpoint);

    let raw_account_data = rpc_client.get_account_data(&address).await?;
    let slot = rpc_client.get_slot().await?;

    let IbcAccountData {
        store: ibc_store, ..
    } = bincode::deserialize(&raw_account_data)?;

    let ibc_state = IbcState::new(&ibc_store, slot);

    let json_str = kind.get_json_str(&ibc_state)?;
    println!("{json_str}");

    Ok(())
}
