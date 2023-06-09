use {
    crate::{eclipse_chain, error::Error, EclipseConsensusState, EclipseHeader},
    core::time::Duration,
    eclipse_ibc_known_proto::{KnownAnyProto, KnownProto, KnownProtoWithFrom},
    eclipse_ibc_proto::eclipse::ibc::chain::v1::ClientState as RawEclipseClientState,
    ibc::core::{
        ics02_client::{
            client_state::{ClientState, UpdateKind, UpdatedState},
            client_type::ClientType,
            consensus_state::ConsensusState,
            error::{ClientError, UpgradeClientError},
            height::Height,
        },
        ics23_commitment::{
            commitment::{CommitmentPrefix, CommitmentProofBytes, CommitmentRoot},
            merkle::MerkleProof,
        },
        ics24_host::{
            identifier::{ChainId, ClientId},
            path::{ClientConsensusStatePath, ClientStatePath, Path, UpgradeClientPath},
        },
        ContextError, ExecutionContext, ValidationContext,
    },
    ibc_proto::{
        google::protobuf,
        ibc::core::commitment::v1::{MerklePath, MerkleProof as RawMerkleProof, MerkleRoot},
        protobuf::Protobuf,
    },
    serde::Serialize,
};

const CLIENT_TYPE: &str = "xx-eclipse";
pub const ECLIPSE_CLIENT_STATE_TYPE_URL: &str = "/eclipse.ibc.v1.chain.ClientState";

fn client_type() -> ClientType {
    ClientType::new(CLIENT_TYPE.to_owned()).unwrap()
}

fn client_err_from_context(err: ContextError) -> ClientError {
    match err {
        ContextError::ClientError(err) => err,
        _ => ClientError::Other {
            description: err.to_string(),
        },
    }
}

// TODO: Store state in a sysvar
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EclipseClientState {
    pub chain_id: ChainId,
    pub latest_header: EclipseHeader,
    pub frozen_height: Option<Height>,
}

impl From<EclipseClientState> for RawEclipseClientState {
    fn from(
        EclipseClientState {
            chain_id,
            latest_header,
            frozen_height,
        }: EclipseClientState,
    ) -> Self {
        Self {
            chain_id: chain_id.to_string(),
            latest_header: Some(latest_header.into()),
            frozen_height: frozen_height.map(Height::into),
        }
    }
}

impl TryFrom<RawEclipseClientState> for EclipseClientState {
    type Error = Error;

    fn try_from(
        RawEclipseClientState {
            chain_id,
            latest_header,
            frozen_height,
        }: RawEclipseClientState,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            chain_id: ChainId::from_string(&chain_id),
            latest_header: latest_header
                .ok_or(Error::MissingFieldInRawClientState {
                    missing_field: "latest_header",
                })?
                .try_into()?,
            frozen_height: frozen_height
                .map(|frozen_height| frozen_height.try_into().map_err(Error::Client))
                .transpose()?,
        })
    }
}

impl Protobuf<RawEclipseClientState> for EclipseClientState {}

impl KnownProtoWithFrom for EclipseClientState {
    type RawWithFrom = RawEclipseClientState;
}

impl KnownAnyProto for EclipseClientState {
    fn type_url() -> String {
        ECLIPSE_CLIENT_STATE_TYPE_URL.to_owned()
    }
}

impl From<EclipseClientState> for protobuf::Any {
    fn from(consensus_state: EclipseClientState) -> Self {
        Self {
            type_url: ECLIPSE_CLIENT_STATE_TYPE_URL.to_owned(),
            value: KnownProto::encode(consensus_state),
        }
    }
}

impl TryFrom<protobuf::Any> for EclipseClientState {
    type Error = ClientError;

    fn try_from(raw: protobuf::Any) -> Result<Self, Self::Error> {
        use prost::Message;

        if &*raw.type_url == ECLIPSE_CLIENT_STATE_TYPE_URL {
            RawEclipseClientState::decode(&*raw.value)
                .map_err(ClientError::Decode)?
                .try_into()
                .map_err(|err: Error| ClientError::ClientSpecific {
                    description: err.to_string(),
                })
        } else {
            Err(ClientError::UnknownClientStateType {
                client_state_type: raw.type_url,
            })
        }
    }
}

impl Protobuf<protobuf::Any> for EclipseClientState {}

impl ClientState for EclipseClientState {
    fn client_type(&self) -> ClientType {
        client_type()
    }

    fn latest_height(&self) -> Height {
        self.latest_header.height
    }

    fn validate_proof_height(&self, proof_height: Height) -> Result<(), ClientError> {
        if proof_height <= self.latest_height() {
            Ok(())
        } else {
            Err(ClientError::InvalidProofHeight {
                latest_height: self.latest_height(),
                proof_height,
            })
        }
    }

    fn confirm_not_frozen(&self) -> Result<(), ClientError> {
        match self.frozen_height {
            None => Ok(()),
            Some(frozen_height) => Err(ClientError::ClientFrozen {
                description: format!("Frozen at height: {frozen_height}"),
            }),
        }
    }

    fn expired(&self, elapsed: Duration) -> bool {
        elapsed > eclipse_chain::IBC_MESSAGE_VALID_DURATION
    }

    fn initialise(
        &self,
        consensus_state: protobuf::Any,
    ) -> Result<Box<dyn ConsensusState>, ClientError> {
        Ok(Box::new(EclipseConsensusState::try_from(consensus_state)?))
    }

    fn verify_client_message(
        &self,
        ctx: &dyn ValidationContext,
        client_id: &ClientId,
        client_message: protobuf::Any,
        update_kind: &UpdateKind,
    ) -> Result<(), ClientError> {
        match update_kind {
            UpdateKind::UpdateClient => (),
            UpdateKind::SubmitMisbehaviour => {
                return Err(ClientError::MisbehaviourHandlingFailure {
                    reason: "Misbehaviour checks are not yet supported".to_owned(),
                });
            }
        }

        let header = EclipseHeader::try_from(client_message)?;

        if self.latest_height() >= header.height {
            return Err(ClientError::LowHeaderHeight {
                header_height: header.height,
                latest_height: self.latest_height(),
            });
        }

        let _client_state = ctx
            .client_state(client_id)
            .map_err(client_err_from_context)?
            .as_any()
            .downcast_ref::<EclipseClientState>()
            .ok_or_else(|| ClientError::ClientSpecific {
                description: "Client state cannot be downcasted into Eclipse client state"
                    .to_owned(),
            })?;

        Ok(())
    }

    // TODO: Support misbehaviour checks
    fn check_for_misbehaviour(
        &self,
        _ctx: &dyn ValidationContext,
        _client_id: &ClientId,
        _client_message: protobuf::Any,
        _update_kind: &UpdateKind,
    ) -> Result<bool, ClientError> {
        Ok(false)
    }

    fn update_state(
        &self,
        ctx: &mut dyn ExecutionContext,
        client_id: &ClientId,
        client_message: protobuf::Any,
    ) -> Result<Vec<Height>, ClientError> {
        let header = EclipseHeader::try_from(client_message)?;
        let new_height = header.height;

        let client_state = ctx
            .client_state(client_id)
            .map_err(client_err_from_context)?
            .as_any()
            .downcast_ref::<EclipseClientState>()
            .ok_or_else(|| ClientError::ClientSpecific {
                description: "Client state cannot be downcasted into Eclipse client state"
                    .to_owned(),
            })?
            .clone();

        let new_client_state = Self {
            chain_id: client_state.chain_id,
            latest_header: header.clone(),
            frozen_height: client_state.frozen_height,
        };

        let new_consensus_state = EclipseConsensusState::from(header);

        ctx.store_update_time(
            client_id.clone(),
            new_client_state.latest_height(),
            ctx.host_timestamp()?,
        )?;
        ctx.store_update_height(
            client_id.clone(),
            new_client_state.latest_height(),
            ctx.host_height()?,
        )?;

        ctx.store_consensus_state(
            ClientConsensusStatePath::new(client_id, &new_client_state.latest_height()),
            Box::new(new_consensus_state),
        )?;
        ctx.store_client_state(ClientStatePath::new(client_id), Box::new(new_client_state))?;

        Ok(vec![new_height])
    }

    fn update_state_on_misbehaviour(
        &self,
        _ctx: &mut dyn ExecutionContext,
        _client_id: &ClientId,
        _client_message: protobuf::Any,
        _update_kind: &UpdateKind,
    ) -> Result<(), ClientError> {
        Err(ClientError::MisbehaviourHandlingFailure {
            reason: "Misbehaviour checks are not yet supported".to_owned(),
        })
    }

    fn verify_upgrade_client(
        &self,
        upgraded_client_state: protobuf::Any,
        upgraded_consensus_state: protobuf::Any,
        proof_upgrade_client: RawMerkleProof,
        proof_upgrade_consensus_state: RawMerkleProof,
        root: &CommitmentRoot,
    ) -> Result<(), ClientError> {
        let upgraded_client_state = EclipseClientState::try_from(upgraded_client_state)?;
        let upgraded_consensus_state = EclipseConsensusState::try_from(upgraded_consensus_state)?;

        let merkle_proof_upgrade_client = MerkleProof::from(proof_upgrade_client);
        let merkle_proof_upgrade_consensus_state = MerkleProof::from(proof_upgrade_consensus_state);

        if self.latest_height() >= upgraded_client_state.latest_height() {
            return Err(UpgradeClientError::LowUpgradeHeight {
                upgraded_height: self.latest_height(),
                client_height: upgraded_client_state.latest_height(),
            }
            .into());
        }

        let last_height = self.latest_height().revision_height();

        let client_upgrade_path = vec![
            //eclipse_chain::UPGRADE_PREFIX.to_owned(),
            UpgradeClientPath::UpgradedClientState(last_height).to_string(),
        ];
        let client_upgrade_merkle_path = MerklePath {
            key_path: client_upgrade_path,
        };

        let client_state_value = KnownProto::encode(upgraded_client_state);

        merkle_proof_upgrade_client
            .verify_membership(
                &eclipse_chain::proof_specs(),
                root.clone().into(),
                client_upgrade_merkle_path,
                client_state_value,
                0,
            )
            .map_err(ClientError::Ics23Verification)?;

        let consensus_upgrade_path = vec![
            //eclipse_chain::UPGRADE_PREFIX.to_owned(),
            UpgradeClientPath::UpgradedClientConsensusState(last_height).to_string(),
        ];
        let consensus_upgrade_merkle_path = MerklePath {
            key_path: consensus_upgrade_path,
        };

        let consensus_state_value = KnownProto::encode(upgraded_consensus_state);

        merkle_proof_upgrade_consensus_state
            .verify_membership(
                &eclipse_chain::proof_specs(),
                root.clone().into(),
                consensus_upgrade_merkle_path,
                consensus_state_value,
                0,
            )
            .map_err(ClientError::Ics23Verification)?;

        Ok(())
    }

    fn update_state_with_upgrade_client(
        &self,
        upgraded_client_state: protobuf::Any,
        upgraded_consensus_state: protobuf::Any,
    ) -> Result<UpdatedState, ClientError> {
        Ok(UpdatedState {
            client_state: Box::new(EclipseClientState::try_from(upgraded_client_state)?),
            consensus_state: Box::new(EclipseConsensusState::try_from(upgraded_consensus_state)?),
        })
    }

    fn verify_membership(
        &self,
        _prefix: &CommitmentPrefix,
        proof: &CommitmentProofBytes,
        root: &CommitmentRoot,
        path: Path,
        value: Vec<u8>,
    ) -> Result<(), ClientError> {
        let proof_specs = eclipse_chain::proof_specs();
        let merkle_root: MerkleRoot = root.clone().into();
        // TODO: Use `ics23_commitment::merkle::apply_prefix`
        let merkle_path = MerklePath {
            key_path: vec![path.to_string()],
        };
        let merkle_proof: MerkleProof = RawMerkleProof::try_from(proof.clone())
            .map_err(ClientError::Ics23Verification)?
            .into();

        merkle_proof
            .verify_membership(&proof_specs, merkle_root, merkle_path, value, 0)
            .map_err(ClientError::Ics23Verification)?;
        Ok(())
    }

    fn verify_non_membership(
        &self,
        _prefix: &CommitmentPrefix,
        proof: &CommitmentProofBytes,
        root: &CommitmentRoot,
        path: Path,
    ) -> Result<(), ClientError> {
        let proof_specs = eclipse_chain::proof_specs();
        let merkle_root: MerkleRoot = root.clone().into();
        // TODO: Use `ics23_commitment::merkle::apply_prefix`
        let merkle_path = MerklePath {
            key_path: vec![path.to_string()],
        };
        let merkle_proof: MerkleProof = RawMerkleProof::try_from(proof.clone())
            .map_err(ClientError::Ics23Verification)?
            .into();

        merkle_proof
            .verify_non_membership(&proof_specs, merkle_root, merkle_path)
            .map_err(ClientError::Ics23Verification)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_client_type() {
        assert_eq!(CLIENT_TYPE, client_type().as_str());
    }
}
