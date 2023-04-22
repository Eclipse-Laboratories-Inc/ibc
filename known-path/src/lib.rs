use {
    core::fmt::Display,
    ibc::core::ics24_host::path::{
        AckPath, ChannelEndPath, ClientConnectionPath, ClientConsensusStatePath, ClientStatePath,
        ClientTypePath, CommitmentPath, ConnectionPath, PortPath, ReceiptPath, SeqAckPath,
        SeqRecvPath, SeqSendPath,
    },
};

/// This is a marker trait for the Merkle store to prevent us from accidentally
/// using a type that is not a path as a key for the store.
pub trait KnownPath: Display {}

impl KnownPath for AckPath {}
impl KnownPath for ChannelEndPath {}
impl KnownPath for ClientConnectionPath {}
impl KnownPath for ClientConsensusStatePath {}
impl KnownPath for ClientStatePath {}
impl KnownPath for ClientTypePath {}
impl KnownPath for CommitmentPath {}
impl KnownPath for ConnectionPath {}
impl KnownPath for PortPath {}
impl KnownPath for ReceiptPath {}
impl KnownPath for SeqAckPath {}
impl KnownPath for SeqRecvPath {}
impl KnownPath for SeqSendPath {}
