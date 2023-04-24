use {
    crate::{error::Error, EclipseConsensusState},
    eclipse_ibc_known_proto::{KnownAnyProto, KnownProto, KnownProtoWithFrom},
    eclipse_ibc_proto::eclipse::ibc::chain::v1::Header as RawEclipseHeader,
    ibc::{
        core::{
            ics02_client::{error::ClientError, header::Header, height::Height},
            ics23_commitment::commitment::CommitmentRoot,
        },
        timestamp::Timestamp,
    },
    ibc_proto::{google::protobuf, protobuf::Protobuf},
    serde::Serialize,
    tendermint::time::Time as TendermintTime,
};

pub const ECLIPSE_HEADER_TYPE_URL: &str = "/eclipse.ibc.v1.chain.Header";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EclipseHeader {
    pub height: Height,
    pub commitment_root: CommitmentRoot,
    pub timestamp: TendermintTime,
}

impl From<EclipseHeader> for RawEclipseHeader {
    fn from(
        EclipseHeader {
            height,
            commitment_root,
            timestamp,
        }: EclipseHeader,
    ) -> Self {
        Self {
            height: Some(height.into()),
            commitment_root: commitment_root.into_vec(),
            timestamp: Some(timestamp.into()),
        }
    }
}

impl TryFrom<RawEclipseHeader> for EclipseHeader {
    type Error = Error;

    fn try_from(
        RawEclipseHeader {
            height,
            commitment_root,
            timestamp,
        }: RawEclipseHeader,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            height: height
                .ok_or(Error::MissingFieldInRawHeader {
                    missing_field: "latest_height",
                })?
                .try_into()
                .map_err(Error::Client)?,
            commitment_root: commitment_root.into(),
            timestamp: timestamp
                .ok_or(Error::MissingFieldInRawHeader {
                    missing_field: "timestamp",
                })?
                .try_into()
                .map_err(Error::Tendermint)?,
        })
    }
}

impl Protobuf<RawEclipseHeader> for EclipseHeader {}

impl KnownProtoWithFrom for EclipseHeader {
    type RawWithFrom = RawEclipseHeader;
}

impl KnownAnyProto for EclipseHeader {
    fn type_url() -> String {
        ECLIPSE_HEADER_TYPE_URL.to_owned()
    }
}

impl From<EclipseHeader> for protobuf::Any {
    fn from(header: EclipseHeader) -> Self {
        Self {
            type_url: ECLIPSE_HEADER_TYPE_URL.to_owned(),
            value: KnownProto::encode(header),
        }
    }
}

impl TryFrom<protobuf::Any> for EclipseHeader {
    type Error = ClientError;

    fn try_from(raw: protobuf::Any) -> Result<Self, Self::Error> {
        use prost::Message;

        if &*raw.type_url == ECLIPSE_HEADER_TYPE_URL {
            RawEclipseHeader::decode(&*raw.value)
                .map_err(ClientError::Decode)?
                .try_into()
                .map_err(|err: Error| ClientError::ClientSpecific {
                    description: err.to_string(),
                })
        } else {
            Err(ClientError::UnknownHeaderType {
                header_type: raw.type_url,
            })
        }
    }
}

impl Protobuf<protobuf::Any> for EclipseHeader {}

impl From<EclipseHeader> for EclipseConsensusState {
    fn from(
        EclipseHeader {
            commitment_root,
            timestamp,
            ..
        }: EclipseHeader,
    ) -> Self {
        Self {
            commitment_root,
            timestamp,
        }
    }
}

impl Header for EclipseHeader {
    fn height(&self) -> Height {
        self.height
    }

    fn timestamp(&self) -> Timestamp {
        self.timestamp.into()
    }
}
