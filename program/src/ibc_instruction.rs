use {
    eclipse_ibc_known_proto::{KnownAnyProto, KnownProto, KnownProtoWithFrom},
    ibc::core::{
        ics02_client::msgs::ClientMsg,
        ics03_connection::msgs::ConnectionMsg,
        ics04_channel::msgs::{ChannelMsg, PacketMsg},
        Msg as _, MsgEnvelope, RouterError,
    },
    ibc_proto::google::protobuf,
    thiserror::Error,
};

pub mod msgs {
    use {
        anyhow::anyhow,
        core::{convert::Infallible, str::FromStr},
        eclipse_ibc_known_proto::{KnownAnyProto, KnownProtoWithFrom},
        eclipse_ibc_proto::eclipse::ibc::{
            admin::v1::{
                msg_write_tx_buffer::Mode as RawMsgWriteTxBufferMode,
                MsgInitStorageAccount as RawMsgInitStorageAccount,
                MsgWriteTxBuffer as RawMsgWriteTxBuffer,
            },
            port::v1::{MsgBindPort as RawMsgBindPort, MsgReleasePort as RawMsgReleasePort},
        },
        ibc::core::ics24_host::identifier::PortId,
    };

    #[derive(Clone, Debug)]
    pub struct MsgBindPort {
        pub port_id: PortId,
    }

    impl MsgBindPort {
        pub const TYPE_URL: &str = "/eclipse.ibc.port.v1.MsgBindPort";
    }

    impl KnownProtoWithFrom for MsgBindPort {
        type RawWithFrom = RawMsgBindPort;
    }

    impl KnownAnyProto for MsgBindPort {
        fn type_url() -> String {
            Self::TYPE_URL.to_owned()
        }
    }

    impl TryFrom<RawMsgBindPort> for MsgBindPort {
        type Error = <PortId as FromStr>::Err;

        fn try_from(RawMsgBindPort { port_id }: RawMsgBindPort) -> Result<Self, Self::Error> {
            let port_id = port_id.parse()?;
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
        pub const TYPE_URL: &str = "/eclipse.ibc.port.v1.MsgReleasePort";
    }

    impl KnownProtoWithFrom for MsgReleasePort {
        type RawWithFrom = RawMsgReleasePort;
    }

    impl KnownAnyProto for MsgReleasePort {
        fn type_url() -> String {
            Self::TYPE_URL.to_owned()
        }
    }

    impl TryFrom<RawMsgReleasePort> for MsgReleasePort {
        type Error = <PortId as FromStr>::Err;

        fn try_from(RawMsgReleasePort { port_id }: RawMsgReleasePort) -> Result<Self, Self::Error> {
            let port_id = port_id.parse()?;
            Ok(Self { port_id })
        }
    }

    impl From<MsgReleasePort> for RawMsgReleasePort {
        fn from(MsgReleasePort { port_id }: MsgReleasePort) -> Self {
            let port_id = port_id.to_string();
            Self { port_id }
        }
    }

    #[derive(Clone, Debug)]
    pub struct MsgInitStorageAccount;

    impl MsgInitStorageAccount {
        pub const TYPE_URL: &str = "/eclipse.ibc.admin.v1.MsgInitStorageAccount";
    }

    impl KnownProtoWithFrom for MsgInitStorageAccount {
        type RawWithFrom = RawMsgInitStorageAccount;
    }

    impl KnownAnyProto for MsgInitStorageAccount {
        fn type_url() -> String {
            Self::TYPE_URL.to_owned()
        }
    }

    impl TryFrom<RawMsgInitStorageAccount> for MsgInitStorageAccount {
        type Error = Infallible;

        fn try_from(
            RawMsgInitStorageAccount {}: RawMsgInitStorageAccount,
        ) -> Result<Self, Self::Error> {
            Ok(Self)
        }
    }

    impl From<MsgInitStorageAccount> for RawMsgInitStorageAccount {
        fn from(MsgInitStorageAccount: MsgInitStorageAccount) -> Self {
            Self {}
        }
    }

    #[derive(Clone, Debug)]
    pub enum MsgWriteTxBufferMode {
        Create { buffer_size: u64 },
        Reuse { offset: u64 },
    }

    impl From<RawMsgWriteTxBufferMode> for MsgWriteTxBufferMode {
        fn from(raw_mode: RawMsgWriteTxBufferMode) -> Self {
            match raw_mode {
                RawMsgWriteTxBufferMode::CreateBufferSize(buffer_size) => {
                    Self::Create { buffer_size }
                }
                RawMsgWriteTxBufferMode::ReuseOffset(offset) => Self::Reuse { offset },
            }
        }
    }

    impl From<MsgWriteTxBufferMode> for RawMsgWriteTxBufferMode {
        fn from(mode: MsgWriteTxBufferMode) -> Self {
            match mode {
                MsgWriteTxBufferMode::Create { buffer_size } => Self::CreateBufferSize(buffer_size),
                MsgWriteTxBufferMode::Reuse { offset } => Self::ReuseOffset(offset),
            }
        }
    }

    #[derive(Clone, Debug)]
    pub struct MsgWriteTxBuffer {
        pub mode: MsgWriteTxBufferMode,
        pub data: Vec<u8>,
    }

    impl MsgWriteTxBuffer {
        pub const TYPE_URL: &str = "/eclipse.ibc.admin.v1.MsgWriteTxBuffer";
    }

    impl KnownProtoWithFrom for MsgWriteTxBuffer {
        type RawWithFrom = RawMsgWriteTxBuffer;
    }

    impl KnownAnyProto for MsgWriteTxBuffer {
        fn type_url() -> String {
            Self::TYPE_URL.to_owned()
        }
    }

    impl TryFrom<RawMsgWriteTxBuffer> for MsgWriteTxBuffer {
        type Error = anyhow::Error;

        fn try_from(
            RawMsgWriteTxBuffer { mode, data }: RawMsgWriteTxBuffer,
        ) -> Result<Self, Self::Error> {
            let mode = mode.ok_or_else(|| anyhow!("Mode cannot be None"))?.into();
            Ok(Self { mode, data })
        }
    }

    impl From<MsgWriteTxBuffer> for RawMsgWriteTxBuffer {
        fn from(MsgWriteTxBuffer { mode, data }: MsgWriteTxBuffer) -> Self {
            Self {
                mode: Some(mode.into()),
                data,
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum PortInstruction {
    Bind(msgs::MsgBindPort),
    Release(msgs::MsgReleasePort),
}

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("the message is malformed and cannot be decoded: {0}")]
    MalformedMessageBytes(anyhow::Error),
    #[error("unknown type URL: {url}")]
    UnknownMessageTypeUrl { url: String },
}

impl KnownProtoWithFrom for PortInstruction {
    type RawWithFrom = protobuf::Any;
}

impl TryFrom<protobuf::Any> for PortInstruction {
    type Error = ProtoError;

    fn try_from(any_msg: protobuf::Any) -> Result<Self, Self::Error> {
        match &*any_msg.type_url {
            msgs::MsgBindPort::TYPE_URL => {
                let msg = msgs::MsgBindPort::decode(&*any_msg.value)
                    .map_err(ProtoError::MalformedMessageBytes)?;
                Ok(Self::Bind(msg))
            }
            msgs::MsgReleasePort::TYPE_URL => {
                let msg = msgs::MsgReleasePort::decode(&*any_msg.value)
                    .map_err(ProtoError::MalformedMessageBytes)?;
                Ok(Self::Release(msg))
            }
            _ => Err(ProtoError::UnknownMessageTypeUrl {
                url: any_msg.type_url,
            }),
        }
    }
}

impl From<PortInstruction> for protobuf::Any {
    fn from(port_instruction: PortInstruction) -> Self {
        match port_instruction {
            PortInstruction::Bind(msg) => msg.encode_as_any(),
            PortInstruction::Release(msg) => msg.encode_as_any(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum AdminInstruction {
    InitStorageAccount(msgs::MsgInitStorageAccount),
    WriteTxBuffer(msgs::MsgWriteTxBuffer),
}

impl KnownProtoWithFrom for AdminInstruction {
    type RawWithFrom = protobuf::Any;
}

impl TryFrom<protobuf::Any> for AdminInstruction {
    type Error = ProtoError;

    fn try_from(any_msg: protobuf::Any) -> Result<Self, Self::Error> {
        match &*any_msg.type_url {
            msgs::MsgInitStorageAccount::TYPE_URL => {
                let msg = msgs::MsgInitStorageAccount::decode(&*any_msg.value)
                    .map_err(ProtoError::MalformedMessageBytes)?;
                Ok(Self::InitStorageAccount(msg))
            }
            msgs::MsgWriteTxBuffer::TYPE_URL => {
                let msg = msgs::MsgWriteTxBuffer::decode(&*any_msg.value)
                    .map_err(ProtoError::MalformedMessageBytes)?;
                Ok(Self::WriteTxBuffer(msg))
            }
            _ => Err(ProtoError::UnknownMessageTypeUrl {
                url: any_msg.type_url,
            }),
        }
    }
}

impl From<AdminInstruction> for protobuf::Any {
    fn from(admin_instruction: AdminInstruction) -> Self {
        match admin_instruction {
            AdminInstruction::InitStorageAccount(msg) => msg.encode_as_any(),
            AdminInstruction::WriteTxBuffer(msg) => msg.encode_as_any(),
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum IbcInstruction {
    Router(MsgEnvelope),
    Port(PortInstruction),
    Admin(AdminInstruction),
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Error)]
pub enum IbcInstructionError {
    #[error(
        "failed to parse IBC instruction; router error: {router_err}; port error: {port_err}; admin error: {admin_err}"
    )]
    UnknownMessageBytes {
        router_err: RouterError,
        port_err: ProtoError,
        admin_err: ProtoError,
    },
}

impl KnownProtoWithFrom for IbcInstruction {
    type RawWithFrom = protobuf::Any;
}

impl TryFrom<protobuf::Any> for IbcInstruction {
    type Error = IbcInstructionError;

    fn try_from(any_msg: protobuf::Any) -> Result<Self, Self::Error> {
        let router_err = match any_msg.clone().try_into() {
            Ok(envelope) => return Ok(Self::Router(envelope)),
            Err(router_err) => router_err,
        };
        let port_err = match any_msg.clone().try_into() {
            Ok(port_instruction) => return Ok(Self::Port(port_instruction)),
            Err(port_err) => port_err,
        };
        let admin_err = match any_msg.try_into() {
            Ok(admin_instruction) => return Ok(Self::Admin(admin_instruction)),
            Err(admin_err) => admin_err,
        };
        Err(IbcInstructionError::UnknownMessageBytes {
            router_err,
            port_err,
            admin_err,
        })
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
                    MsgEnvelope::Client(ClientMsg::Misbehaviour(domain_msg)) => domain_msg.to_any(),
                    MsgEnvelope::Client(ClientMsg::UpgradeClient(domain_msg)) => {
                        domain_msg.to_any()
                    }

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
            IbcInstruction::Admin(admin_instruction) => admin_instruction.into(),
        }
    }
}
