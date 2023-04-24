use {
    core::fmt::Display,
    eclipse_ibc_extra_types::ClientConnections,
    eclipse_ibc_known_proto::KnownProto,
    ibc::core::{
        ics03_connection::connection::ConnectionEnd,
        ics04_channel::{
            channel::ChannelEnd,
            commitment::{AcknowledgementCommitment, PacketCommitment},
            packet::{Receipt, Sequence},
        },
        ics24_host::path::{
            AckPath, ChannelEndPath, ClientConnectionPath, ClientConsensusStatePath,
            ClientStatePath, CommitmentPath, ConnectionPath, PortPath, ReceiptPath, SeqAckPath,
            SeqRecvPath, SeqSendPath,
        },
        ics26_routing::context::ModuleId,
    },
    ibc_proto::google::protobuf,
};

/// This is a marker trait for the Merkle store to prevent us from accidentally
/// using a type that is not a path as a key for the store.
pub trait KnownPath: Display {
    type Value: KnownProto;
}

impl KnownPath for AckPath {
    type Value = AcknowledgementCommitment;
}

impl KnownPath for ChannelEndPath {
    type Value = ChannelEnd;
}

impl KnownPath for ClientConnectionPath {
    type Value = ClientConnections;
}

impl KnownPath for ClientConsensusStatePath {
    type Value = protobuf::Any;
}

impl KnownPath for ClientStatePath {
    type Value = protobuf::Any;
}

impl KnownPath for CommitmentPath {
    type Value = PacketCommitment;
}

impl KnownPath for ConnectionPath {
    type Value = ConnectionEnd;
}

impl KnownPath for PortPath {
    type Value = ModuleId;
}

impl KnownPath for ReceiptPath {
    type Value = Receipt;
}

impl KnownPath for SeqAckPath {
    type Value = Sequence;
}

impl KnownPath for SeqRecvPath {
    type Value = Sequence;
}

impl KnownPath for SeqSendPath {
    type Value = Sequence;
}
