use {
    ibc::core::ics02_client::error::ClientError, tendermint::error::Error as TendermintError,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid raw consensus state due to a missing field: {missing_field}")]
    MissingFieldInRawConsensusState { missing_field: &'static str },
    #[error("invalid raw header due to a missing field: {missing_field}")]
    MissingFieldInRawHeader { missing_field: &'static str },
    #[error("invalid raw client state due to a missing field: {missing_field}")]
    MissingFieldInRawClientState { missing_field: &'static str },
    #[error("Tendermint error: {0}")]
    Tendermint(TendermintError),
    #[error("IBC client error: {0}")]
    Client(ClientError),
}
