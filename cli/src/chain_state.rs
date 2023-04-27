use {
    anyhow::anyhow,
    eclipse_ibc_light_client::{
        eclipse_chain, EclipseClientState, EclipseConsensusState, EclipseHeader,
    },
    eclipse_ibc_state::{IbcAccountData, IbcState},
    ibc::core::ics02_client::height::Height,
    solana_client::nonblocking::rpc_client::RpcClient,
    tendermint::time::Time as TendermintTime,
};

pub(crate) async fn get_consensus_state(
    rpc_client: &RpcClient,
    height: Height,
) -> anyhow::Result<EclipseConsensusState> {
    let slot = eclipse_chain::slot_of_height(height)?;
    let block = rpc_client.get_block(slot).await?;

    let raw_account_data = rpc_client
        .get_account_data(&eclipse_ibc_program::STORAGE_KEY)
        .await?;

    let IbcAccountData {
        store: ibc_store, ..
    } = bincode::deserialize(&raw_account_data)?;

    let version = ibc_store
        .read()?
        .find_version(slot)
        .ok_or_else(|| anyhow!("No IBC state versions found"))?;
    let ibc_state = IbcState::new(&ibc_store, version);
    let commitment_root = ibc_state
        .get_root_option(version)?
        .ok_or_else(|| anyhow!("No commitment root found for slot {slot}"))?;

    let timestamp = TendermintTime::from_unix_timestamp(
        block
            .block_time
            .ok_or_else(|| anyhow!("Block timestamp should not be missing"))?,
        0,
    )
    .expect("Block time should be valid");

    Ok(EclipseConsensusState {
        commitment_root,
        timestamp,
    })
}

pub(crate) fn header_from_consensus_state(
    EclipseConsensusState {
        commitment_root,
        timestamp,
    }: EclipseConsensusState,
    height: Height,
) -> EclipseHeader {
    EclipseHeader {
        height,
        commitment_root,
        timestamp,
    }
}

pub(crate) fn client_state_from_header(
    latest_header: EclipseHeader,
    chain_name: &str,
) -> EclipseClientState {
    EclipseClientState {
        chain_id: eclipse_chain::chain_id(chain_name),
        latest_header,
        frozen_height: None,
    }
}
