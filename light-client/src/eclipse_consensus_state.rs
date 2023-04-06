use {
    crate::error::Error,
    eclipse_ibc_proto::eclipse::ibc::chain::v1::ConsensusState as RawEclipseConsensusState,
    ibc::{
        core::{
            ics02_client::{consensus_state::ConsensusState, error::ClientError},
            ics23_commitment::commitment::CommitmentRoot,
        },
        timestamp::Timestamp,
    },
    ibc_proto::{google::protobuf, protobuf::Protobuf},
    known_proto::{KnownProto, KnownProtoWithFrom},
    serde::Serialize,
    tendermint::time::Time as TendermintTime,
};

pub const ECLIPSE_CONSENSUS_STATE_TYPE_URL: &str = "/eclipse.ibc.v1.chain.ConsensusState";

// TODO: Store state in a sysvar
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EclipseConsensusState {
    pub commitment_root: CommitmentRoot,
    pub timestamp: TendermintTime,
}

impl ConsensusState for EclipseConsensusState {
    fn root(&self) -> &CommitmentRoot {
        &self.commitment_root
    }

    fn timestamp(&self) -> Timestamp {
        self.timestamp.into()
    }
}

impl From<EclipseConsensusState> for RawEclipseConsensusState {
    fn from(
        EclipseConsensusState {
            commitment_root,
            timestamp,
        }: EclipseConsensusState,
    ) -> Self {
        Self {
            commitment_root: commitment_root.into_vec(),
            timestamp: Some(timestamp.into()),
        }
    }
}

impl TryFrom<RawEclipseConsensusState> for EclipseConsensusState {
    type Error = Error;

    fn try_from(
        RawEclipseConsensusState {
            commitment_root,
            timestamp,
        }: RawEclipseConsensusState,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            commitment_root: commitment_root.into(),
            timestamp: timestamp
                .ok_or(Error::MissingFieldInRawConsensusState {
                    missing_field: "timestamp",
                })?
                .try_into()
                .map_err(Error::Tendermint)?,
        })
    }
}

impl Protobuf<RawEclipseConsensusState> for EclipseConsensusState {}

impl KnownProtoWithFrom for EclipseConsensusState {
    type RawWithFrom = RawEclipseConsensusState;
}

impl From<EclipseConsensusState> for protobuf::Any {
    fn from(consensus_state: EclipseConsensusState) -> Self {
        Self {
            type_url: ECLIPSE_CONSENSUS_STATE_TYPE_URL.to_owned(),
            value: KnownProto::encode(consensus_state),
        }
    }
}

impl TryFrom<protobuf::Any> for EclipseConsensusState {
    type Error = ClientError;

    fn try_from(raw: protobuf::Any) -> Result<Self, Self::Error> {
        use prost::Message;

        if &*raw.type_url == ECLIPSE_CONSENSUS_STATE_TYPE_URL {
            RawEclipseConsensusState::decode(&*raw.value)
                .map_err(ClientError::Decode)?
                .try_into()
                .map_err(|err: Error| ClientError::ClientSpecific {
                    description: err.to_string(),
                })
        } else {
            Err(ClientError::UnknownConsensusStateType {
                consensus_state_type: raw.type_url,
            })
        }
    }
}

impl Protobuf<protobuf::Any> for EclipseConsensusState {}
