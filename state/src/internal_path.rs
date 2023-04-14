use {
    derive_more::Display,
    ibc::core::{ics02_client::height::Height, ics24_host::identifier::ClientId},
};

#[derive(Clone, Debug, Display, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[display(fmt = "internal/clients/{_0}/updateTime/{_1}")]
pub struct ClientUpdateTimePath(pub ClientId, pub Height);

#[derive(Clone, Debug, Display, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[display(fmt = "internal/clients/{_0}/updateHeight/{_1}")]
pub struct ClientUpdateHeightPath(pub ClientId, pub Height);

/// Keeps track of a sorted list of known consensus heights so that `prev_consensus_state`
/// and `next_consensus_state` can be implemented.
#[derive(Clone, Debug, Display, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[display(fmt = "internal/clients/{_0}/consensusHeights")]
pub struct ConsensusHeightsPath(pub ClientId);

/// Keeps track of all modules that have bound to a port. This is due to a limitation
/// with the ibc-rs interface for the `Router`, which must instantiate all modules
/// ahead of time.
#[derive(Clone, Debug, Display, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[display(fmt = "internal/allModules")]
pub struct AllModulesPath;