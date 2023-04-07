pub mod eclipse_chain;
mod eclipse_client_state;
mod eclipse_consensus_state;
mod eclipse_header;
mod error;

use eclipse_header::EclipseHeader;
pub use {
    eclipse_client_state::{EclipseClientState, ECLIPSE_CLIENT_STATE_TYPE_URL},
    eclipse_consensus_state::{EclipseConsensusState, ECLIPSE_CONSENSUS_STATE_TYPE_URL},
};
