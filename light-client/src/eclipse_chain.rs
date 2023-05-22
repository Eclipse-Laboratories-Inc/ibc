use {
    core::time::Duration,
    ibc::core::{
        ics02_client::{error::ClientError, height::Height},
        ics23_commitment::specs::ProofSpecs,
        ics24_host::identifier::ChainId,
    },
    solana_sdk::{clock::Slot, sysvar::clock::Clock},
    tendermint::time::Time as TendermintTime,
};

/// Target slot time is 400ms but in practice Solana goes up to 600ms
pub const MAX_EXPECTED_SLOT_TIME: Duration = Duration::from_millis(600);
pub const IBC_MESSAGE_VALID_DURATION: Duration = Duration::from_secs(3600);
pub const CHAIN_NAME_PREFIX: &str = "eclipse";
pub const UPGRADE_PREFIX: &str = "eclipse-upgrade";
pub const COMMITMENT_PREFIX: &[u8] = b"ibc";
const REVISION_NUMBER: u64 = 0;

pub fn chain_id(chain_name: &str) -> ChainId {
    ChainId::new(
        &format!("{CHAIN_NAME_PREFIX}-{chain_name}"),
        REVISION_NUMBER,
    )
}

pub fn height_of_slot(slot: Slot) -> Result<Height, ClientError> {
    // clock.slot starts at 0, so we add 1 for the height
    let revision_height = slot
        .checked_add(1)
        .ok_or_else(|| ClientError::InvalidHeight)?;
    Height::new(REVISION_NUMBER, revision_height)
}

pub fn slot_of_height(height: Height) -> Result<Slot, ClientError> {
    if height.revision_number() != REVISION_NUMBER {
        return Err(ClientError::InvalidHeight);
    }

    // clock.slot starts at 0, so we subtract 1 from the height
    height
        .revision_height()
        .checked_sub(1)
        .ok_or_else(|| ClientError::InvalidHeight)
}

pub fn tendermint_time_from_clock(clock: &Clock) -> TendermintTime {
    TendermintTime::from_unix_timestamp(clock.unix_timestamp, 0)
        .expect("Unix timestamp from Clock should be valid")
}

pub fn proof_specs() -> ProofSpecs {
    // TODO: Figure out top-level proof spec to use
    vec![jmt::ics23_spec()].into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_id_to_string() {
        assert_eq!(chain_id("apricot").to_string(), "eclipse-apricot-0");
    }

    #[test]
    fn height_of_slot_to_string() {
        assert_eq!(height_of_slot(0).unwrap().to_string(), "0-1");
        assert_eq!(height_of_slot(5).unwrap().to_string(), "0-6");
    }

    #[test]
    fn check_slot_of_height() {
        assert_eq!(
            slot_of_height(Height::new(REVISION_NUMBER, 1).unwrap()).unwrap(),
            0,
        );
        assert!(slot_of_height(Height::new(REVISION_NUMBER + 1, 1).unwrap()).is_err());
    }
}
