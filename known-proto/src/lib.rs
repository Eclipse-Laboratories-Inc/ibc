use {
    anyhow::{anyhow, bail, Context as _},
    bytes::Buf,
    ibc::{
        clients::ics07_tendermint::{
            client_state::{
                ClientState as TendermintClientState, TENDERMINT_CLIENT_STATE_TYPE_URL,
            },
            consensus_state::{
                ConsensusState as TendermintConsensusState, TENDERMINT_CONSENSUS_STATE_TYPE_URL,
            },
        },
        core::{
            ics02_client::{client_type::ClientType, height::Height},
            ics03_connection::connection::ConnectionEnd,
            ics04_channel::{
                channel::ChannelEnd,
                commitment::{AcknowledgementCommitment, PacketCommitment},
                packet::{Receipt, Sequence},
            },
            ics24_host::identifier::ConnectionId,
            ics26_routing::context::{InvalidModuleId, ModuleId},
        },
    },
    ibc_proto::{
        google::protobuf,
        ibc::core::{
            channel::v1::Channel as RawChannel, client::v1::Height as RawHeight,
            connection::v1::ConnectionEnd as RawConnectionEnd,
        },
    },
    prost::Message as _,
};

pub trait KnownProto
where
    Self: Sized,
    Self::Raw: Default + prost::Message,
{
    type Raw;

    // These functions are needed because we cannot derive `From` and `TryFrom`
    // on foreign types.
    fn into_raw(self) -> Self::Raw;
    fn from_raw(raw: Self::Raw) -> anyhow::Result<Self>;

    fn encode(self) -> Vec<u8> {
        self.into_raw().encode_to_vec()
    }

    fn decode<B: Buf>(buf: B) -> anyhow::Result<Self> {
        Self::from_raw(Self::Raw::decode(buf).context("error decoding buffer into message")?)
    }
}

pub trait KnownProtoWithFrom
where
    Self: TryFrom<Self::RawWithFrom> + Sized,
    Self::RawWithFrom: From<Self> + Default + prost::Message,
{
    type RawWithFrom;
}

impl<T: KnownProtoWithFrom> KnownProto for T
where
    <T as TryFrom<T::RawWithFrom>>::Error: Into<anyhow::Error>,
{
    type Raw = T::RawWithFrom;

    #[inline]
    fn into_raw(self) -> Self::Raw {
        self.into()
    }

    #[inline]
    fn from_raw(raw: Self::Raw) -> anyhow::Result<Self> {
        raw.try_into().map_err(Into::into)
    }
}

pub trait KnownAnyProto
where
    Self: KnownProto,
{
    fn type_url() -> String;

    fn encode_as_any(self) -> protobuf::Any {
        protobuf::Any {
            type_url: Self::type_url(),
            value: self.encode(),
        }
    }
}

impl KnownProto for ClientType {
    type Raw = String;

    fn into_raw(self) -> Self::Raw {
        self.as_str().to_owned()
    }

    fn from_raw(raw: Self::Raw) -> anyhow::Result<Self> {
        Ok(Self::new(raw)?)
    }
}

impl KnownProtoWithFrom for TendermintClientState {
    type RawWithFrom = protobuf::Any;
}

impl KnownAnyProto for TendermintClientState {
    fn type_url() -> String {
        TENDERMINT_CLIENT_STATE_TYPE_URL.to_owned()
    }
}

impl KnownProtoWithFrom for TendermintConsensusState {
    type RawWithFrom = protobuf::Any;
}

impl KnownAnyProto for TendermintConsensusState {
    fn type_url() -> String {
        TENDERMINT_CONSENSUS_STATE_TYPE_URL.to_owned()
    }
}

impl KnownProtoWithFrom for ConnectionEnd {
    type RawWithFrom = RawConnectionEnd;
}

impl KnownProto for ConnectionId {
    type Raw = String;

    fn into_raw(self) -> Self::Raw {
        self.as_str().to_owned()
    }

    fn from_raw(raw: Self::Raw) -> anyhow::Result<Self> {
        Ok(raw.parse()?)
    }
}

impl KnownProto for PacketCommitment {
    type Raw = Vec<u8>;

    fn into_raw(self) -> Self::Raw {
        self.into_vec()
    }

    fn from_raw(raw: Self::Raw) -> anyhow::Result<Self> {
        Ok(raw.into())
    }
}

impl KnownProto for Receipt {
    type Raw = Vec<u8>;

    fn into_raw(self) -> Self::Raw {
        match self {
            Self::Ok => vec![1],
        }
    }

    fn from_raw(raw: Self::Raw) -> anyhow::Result<Self> {
        match &raw[..] {
            [1] => Ok(Self::Ok),
            _ => bail!("invalid packet receipt: {raw:?}"),
        }
    }
}

impl KnownProto for AcknowledgementCommitment {
    type Raw = Vec<u8>;

    fn into_raw(self) -> Self::Raw {
        self.into_vec()
    }

    fn from_raw(raw: Self::Raw) -> anyhow::Result<Self> {
        Ok(raw.into())
    }
}

impl KnownProtoWithFrom for ChannelEnd {
    type RawWithFrom = RawChannel;
}

impl KnownProtoWithFrom for Sequence {
    type RawWithFrom = u64;
}

impl KnownProtoWithFrom for () {
    type RawWithFrom = ();
}

impl KnownProtoWithFrom for protobuf::Any {
    type RawWithFrom = Self;
}

impl KnownProtoWithFrom for tendermint::time::Time {
    type RawWithFrom = tendermint_proto::google::protobuf::Timestamp;
}

impl KnownProto for ibc::timestamp::Timestamp {
    type Raw = u64;

    fn into_raw(self) -> Self::Raw {
        self.nanoseconds()
    }

    fn from_raw(raw: Self::Raw) -> anyhow::Result<Self> {
        Ok(ibc::timestamp::Timestamp::from_nanoseconds(raw)?)
    }
}

impl KnownProtoWithFrom for Height {
    type RawWithFrom = RawHeight;
}

impl KnownProto for ModuleId {
    type Raw = String;

    fn into_raw(self) -> Self::Raw {
        self.to_string()
    }

    fn from_raw(raw: Self::Raw) -> anyhow::Result<Self> {
        raw.parse()
            .map_err(|InvalidModuleId| anyhow!("Invalid module ID: {:?}", raw))
    }
}
