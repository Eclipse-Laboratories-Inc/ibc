use {
    eclipse_ibc_proto::eclipse::ibc::client::v1::ConsensusHeights as RawConsensusHeights,
    eclipse_known_proto::KnownProtoWithFrom,
    ibc::core::ics02_client::{error::ClientError, height::Height},
    ibc_proto::ibc::core::client::v1::Height as RawHeight,
    std::collections::BTreeSet,
};

#[derive(Clone, Debug, Default)]
pub struct ConsensusHeights {
    pub heights: BTreeSet<Height>,
}

impl From<ConsensusHeights> for RawConsensusHeights {
    fn from(ConsensusHeights { heights }: ConsensusHeights) -> Self {
        Self {
            heights: heights.into_iter().map(Height::into).collect(),
        }
    }
}

impl TryFrom<RawConsensusHeights> for ConsensusHeights {
    type Error = ClientError;

    fn try_from(RawConsensusHeights { heights }: RawConsensusHeights) -> Result<Self, Self::Error> {
        Ok(Self {
            heights: heights
                .into_iter()
                .map(RawHeight::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl KnownProtoWithFrom for ConsensusHeights {
    type RawWithFrom = RawConsensusHeights;
}
