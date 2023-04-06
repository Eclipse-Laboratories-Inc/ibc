use {
    ibc::{
        core::{
            ics02_client::msgs::ClientMsg,
            ics03_connection::msgs::ConnectionMsg,
            ics04_channel::msgs::{ChannelMsg, PacketMsg},
            ics26_routing::{error::RouterError, msgs::MsgEnvelope},
        },
        tx_msg::Msg as _,
    },
    ibc_proto::google::protobuf,
    known_proto::{KnownProto, KnownProtoWithFrom},
    thiserror::Error,
};

pub(super) mod msgs {
    use {
        eclipse_ibc_proto::eclipse::ibc::port::v1::{
            MsgBindPort as RawMsgBindPort, MsgReleasePort as RawMsgReleasePort,
        },
        ibc::{core::ics24_host::identifier::PortId, tx_msg::Msg},
        known_proto::KnownProtoWithFrom,
        std::str::FromStr,
    };

    #[derive(Clone, Debug)]
    pub struct MsgBindPort {
        pub port_id: PortId,
    }

    impl MsgBindPort {
        pub(super) const TYPE_URL: &str = "/eclipse.ibc.v1.port.MsgBindPort";
    }

    impl Msg for MsgBindPort {
        type Raw = RawMsgBindPort;

        fn type_url(&self) -> String {
            Self::TYPE_URL.to_owned()
        }
    }

    impl KnownProtoWithFrom for MsgBindPort {
        type RawWithFrom = RawMsgBindPort;
    }

    impl TryFrom<RawMsgBindPort> for MsgBindPort {
        type Error = <PortId as FromStr>::Err;

        fn try_from(RawMsgBindPort { port_id }: RawMsgBindPort) -> Result<Self, Self::Error> {
            let port_id = PortId::from_str(&port_id)?;
            Ok(Self { port_id })
        }
    }

    impl From<MsgBindPort> for RawMsgBindPort {
        fn from(MsgBindPort { port_id }: MsgBindPort) -> Self {
            let port_id = port_id.to_string();
            Self { port_id }
        }
    }

    #[derive(Clone, Debug)]
    pub struct MsgReleasePort {
        pub port_id: PortId,
    }

    impl MsgReleasePort {
        pub(super) const TYPE_URL: &str = "/eclipse.ibc.v1.port.MsgReleasePort";
    }

    impl Msg for MsgReleasePort {
        type Raw = RawMsgReleasePort;

        fn type_url(&self) -> String {
            Self::TYPE_URL.to_owned()
        }
    }

    impl KnownProtoWithFrom for MsgReleasePort {
        type RawWithFrom = RawMsgReleasePort;
    }

    impl TryFrom<RawMsgReleasePort> for MsgReleasePort {
        type Error = <PortId as FromStr>::Err;

        fn try_from(RawMsgReleasePort { port_id }: RawMsgReleasePort) -> Result<Self, Self::Error> {
            let port_id = PortId::from_str(&port_id)?;
            Ok(Self { port_id })
        }
    }

    impl From<MsgReleasePort> for RawMsgReleasePort {
        fn from(MsgReleasePort { port_id }: MsgReleasePort) -> Self {
            let port_id = port_id.to_string();
            Self { port_id }
        }
    }
}

#[derive(Clone, Debug)]
pub enum PortInstruction {
    Bind(msgs::MsgBindPort),
    Release(msgs::MsgReleasePort),
}

#[derive(Debug, Error)]
pub enum PortInstructionError {
    #[error("the message is malformed and cannot be decoded: {0}")]
    MalformedMessageBytes(anyhow::Error),
    #[error("unknown type URL: {url}")]
    UnknownMessageTypeUrl { url: String },
}

impl KnownProtoWithFrom for PortInstruction {
    type RawWithFrom = protobuf::Any;
}

impl TryFrom<protobuf::Any> for PortInstruction {
    type Error = PortInstructionError;

    fn try_from(any_msg: protobuf::Any) -> Result<Self, Self::Error> {
        match &*any_msg.type_url {
            msgs::MsgBindPort::TYPE_URL => {
                let msg = msgs::MsgBindPort::decode(&*any_msg.value)
                    .map_err(PortInstructionError::MalformedMessageBytes)?;
                Ok(Self::Bind(msg))
            }
            msgs::MsgReleasePort::TYPE_URL => {
                let msg = msgs::MsgReleasePort::decode(&*any_msg.value)
                    .map_err(PortInstructionError::MalformedMessageBytes)?;
                Ok(Self::Release(msg))
            }
            _ => Err(PortInstructionError::UnknownMessageTypeUrl {
                url: any_msg.type_url,
            }),
        }
    }
}

impl From<PortInstruction> for protobuf::Any {
    fn from(port_instruction: PortInstruction) -> Self {
        match port_instruction {
            PortInstruction::Bind(msg_bind_port) => Self {
                type_url: msgs::MsgBindPort::TYPE_URL.to_owned(),
                value: msg_bind_port.encode(),
            },
            PortInstruction::Release(msg_release_port) => Self {
                type_url: msgs::MsgReleasePort::TYPE_URL.to_owned(),
                value: msg_release_port.encode(),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub enum IbcInstruction {
    Router(MsgEnvelope),
    Port(PortInstruction),
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Error)]
pub enum IbcInstructionError {
    #[error("failed to parse IBC instruction; router error: {router_err}; port error: {port_err}")]
    UnknownMessageBytes {
        router_err: RouterError,
        port_err: PortInstructionError,
    },
}

impl KnownProtoWithFrom for IbcInstruction {
    type RawWithFrom = protobuf::Any;
}

impl TryFrom<protobuf::Any> for IbcInstruction {
    type Error = IbcInstructionError;

    fn try_from(any_msg: protobuf::Any) -> Result<Self, Self::Error> {
        match any_msg.clone().try_into() {
            Ok(envelope) => Ok(Self::Router(envelope)),
            Err(router_err) => match any_msg.try_into() {
                Ok(port_instruction) => Ok(Self::Port(port_instruction)),
                Err(port_err) => Err(IbcInstructionError::UnknownMessageBytes {
                    router_err,
                    port_err,
                }),
            },
        }
    }
}

impl From<IbcInstruction> for protobuf::Any {
    fn from(ibc_instruction: IbcInstruction) -> Self {
        match ibc_instruction {
            IbcInstruction::Router(msg_envelope) => {
                match msg_envelope {
                    // ICS2 messages
                    MsgEnvelope::Client(ClientMsg::CreateClient(domain_msg)) => domain_msg.to_any(),
                    MsgEnvelope::Client(ClientMsg::UpdateClient(domain_msg)) => domain_msg.to_any(),
                    MsgEnvelope::Client(ClientMsg::UpgradeClient(domain_msg)) => {
                        domain_msg.to_any()
                    }
                    MsgEnvelope::Client(ClientMsg::Misbehaviour(domain_msg)) => domain_msg.to_any(),

                    // ICS03
                    MsgEnvelope::Connection(ConnectionMsg::OpenInit(domain_msg)) => {
                        domain_msg.to_any()
                    }
                    MsgEnvelope::Connection(ConnectionMsg::OpenTry(domain_msg)) => {
                        domain_msg.to_any()
                    }
                    MsgEnvelope::Connection(ConnectionMsg::OpenAck(domain_msg)) => {
                        domain_msg.to_any()
                    }
                    MsgEnvelope::Connection(ConnectionMsg::OpenConfirm(domain_msg)) => {
                        domain_msg.to_any()
                    }

                    // ICS04 channel messages
                    MsgEnvelope::Channel(ChannelMsg::OpenInit(domain_msg)) => domain_msg.to_any(),
                    MsgEnvelope::Channel(ChannelMsg::OpenTry(domain_msg)) => domain_msg.to_any(),
                    MsgEnvelope::Channel(ChannelMsg::OpenAck(domain_msg)) => domain_msg.to_any(),
                    MsgEnvelope::Channel(ChannelMsg::OpenConfirm(domain_msg)) => {
                        domain_msg.to_any()
                    }
                    MsgEnvelope::Channel(ChannelMsg::CloseInit(domain_msg)) => domain_msg.to_any(),
                    MsgEnvelope::Channel(ChannelMsg::CloseConfirm(domain_msg)) => {
                        domain_msg.to_any()
                    }
                    // ICS04 packet messages
                    MsgEnvelope::Packet(PacketMsg::Recv(domain_msg)) => domain_msg.to_any(),
                    MsgEnvelope::Packet(PacketMsg::Ack(domain_msg)) => domain_msg.to_any(),
                    MsgEnvelope::Packet(PacketMsg::Timeout(domain_msg)) => domain_msg.to_any(),
                    MsgEnvelope::Packet(PacketMsg::TimeoutOnClose(domain_msg)) => {
                        domain_msg.to_any()
                    }
                }
            }
            IbcInstruction::Port(port_instruction) => port_instruction.into(),
        }
    }
}
