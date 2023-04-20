use {
    eclipse_ibc_light_client::{
        EclipseClientState, EclipseConsensusState, ECLIPSE_CLIENT_STATE_TYPE_URL,
        ECLIPSE_CONSENSUS_STATE_TYPE_URL,
    },
    eclipse_ibc_proto::eclipse::ibc::chain::v1::{
        ClientState as RawEclipseClientState, ConsensusState as RawEclipseConsensusState,
    },
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
            context::ContextError,
            ics02_client::{
                client_state::ClientState, consensus_state::ConsensusState, error::ClientError,
            },
        },
    },
    ibc_proto::{
        google::protobuf,
        ibc::lightclients::tendermint::v1::{
            ClientState as RawTmClientState, ConsensusState as RawTmConsensusState,
        },
        protobuf::Protobuf,
    },
    known_proto::KnownAnyProto,
};

pub fn decode_client_state(
    client_state: protobuf::Any,
) -> Result<Box<dyn ClientState>, ContextError> {
    match &*client_state.type_url {
        TENDERMINT_CLIENT_STATE_TYPE_URL => Ok(Box::new(
            <TendermintClientState as Protobuf<RawTmClientState>>::decode_vec(&client_state.value)
                .map_err(|err| ClientError::Other {
                    description: err.to_string(),
                })?,
        )),
        ECLIPSE_CLIENT_STATE_TYPE_URL => Ok(Box::new(
            <EclipseClientState as Protobuf<RawEclipseClientState>>::decode_vec(
                &client_state.value,
            )
            .map_err(|err| ClientError::Other {
                description: err.to_string(),
            })?,
        )),
        _ => Err(ClientError::UnknownClientStateType {
            client_state_type: client_state.type_url,
        }
        .into()),
    }
}

pub fn encode_client_state(
    client_state: Box<dyn ClientState>,
) -> Result<protobuf::Any, ContextError> {
    if let Some(client_state) = client_state
        .as_any()
        .downcast_ref::<TendermintClientState>()
    {
        Ok(client_state.clone().encode_as_any())
    } else if let Some(client_state) = client_state.as_any().downcast_ref::<EclipseClientState>() {
        Ok(client_state.clone().encode_as_any())
    } else {
        Err(ClientError::Other {
            description: format!(
                "could not downcast client state to specific type; client type: {}",
                client_state.client_type(),
            ),
        }
        .into())
    }
}

pub fn decode_consensus_state(
    consensus_state: protobuf::Any,
) -> Result<Box<dyn ConsensusState>, ContextError> {
    match &*consensus_state.type_url {
        TENDERMINT_CONSENSUS_STATE_TYPE_URL => Ok(Box::new(
            <TendermintConsensusState as Protobuf<RawTmConsensusState>>::decode_vec(
                &consensus_state.value,
            )
            .map_err(|err| ClientError::Other {
                description: err.to_string(),
            })?,
        )),
        ECLIPSE_CONSENSUS_STATE_TYPE_URL => Ok(Box::new(
            <EclipseConsensusState as Protobuf<RawEclipseConsensusState>>::decode_vec(
                &consensus_state.value,
            )
            .map_err(|err| ClientError::Other {
                description: err.to_string(),
            })?,
        )),
        _ => Err(ClientError::UnknownConsensusStateType {
            consensus_state_type: consensus_state.type_url,
        }
        .into()),
    }
}

pub fn encode_consensus_state(
    consensus_state: Box<dyn ConsensusState>,
) -> Result<protobuf::Any, ContextError> {
    if let Some(consensus_state) = consensus_state
        .as_any()
        .downcast_ref::<TendermintConsensusState>()
    {
        Ok(consensus_state.clone().encode_as_any())
    } else if let Some(consensus_state) = consensus_state
        .as_any()
        .downcast_ref::<EclipseConsensusState>()
    {
        Ok(consensus_state.clone().encode_as_any())
    } else {
        Err(ClientError::Other {
            description: "could not downcast consensus state to specific type".to_owned(),
        }
        .into())
    }
}
