use {
    ibc::{
        applications::transfer::{
            amount::Amount, coin::PrefixedCoin, denom::PrefixedDenom, error::TokenTransferError,
        },
        core::ics24_host::{
            error::ValidationError,
            identifier::{ChannelId, PortId},
        },
        signer::Signer,
    },
    serde::{Deserialize, Serialize},
    std::collections::{BTreeMap, HashMap},
};

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct Ics20Module {
    port: Option<PortId>,
    signer_amt_by_token: BTreeMap<PrefixedDenom, HashMap<Signer, Amount>>,
    is_send_enabled: bool,
    is_receive_enabled: bool,
}

impl Ics20Module {
    pub(super) fn _bind_port(&mut self, port: PortId) {
        let _old_port = self.port.insert(port);
    }
}

// impl BankKeeper for Ics20Module
impl Ics20Module {
    fn _send_coins(
        &mut self,
        _from: &Signer,
        _to: &Signer,
        _amt: &PrefixedCoin,
    ) -> Result<(), TokenTransferError> {
        todo!()
    }

    fn _mint_coins(
        &mut self,
        account: &Signer,
        amt: &PrefixedCoin,
    ) -> Result<(), TokenTransferError> {
        self.signer_amt_by_token
            .entry(amt.denom.clone())
            .or_default()
            .entry(account.clone())
            .or_insert_with(|| 0u64.into())
            .checked_add(amt.amount)
            .ok_or_else(|| TokenTransferError::InvalidToken)?;
        Ok(())
    }

    fn _burn_coins(
        &mut self,
        account: &Signer,
        amt: &PrefixedCoin,
    ) -> Result<(), TokenTransferError> {
        self.signer_amt_by_token
            .get_mut(&amt.denom)
            .ok_or_else(|| TokenTransferError::InvalidToken)?
            .get_mut(account)
            .ok_or_else(|| TokenTransferError::InvalidToken)?
            .checked_sub(amt.amount)
            .ok_or_else(|| TokenTransferError::InvalidToken)?;
        Ok(())
    }
}

// impl Ics20Reader for Ics20Module
impl Ics20Module {
    fn _get_port(&self) -> Result<PortId, TokenTransferError> {
        Ok(self
            .port
            .as_ref()
            .ok_or_else(|| TokenTransferError::InvalidPortId {
                context: String::new(),
                validation_error: ValidationError::Empty,
            })?
            .clone())
    }

    fn _get_channel_escrow_address(
        &self,
        _port_id: &PortId,
        _channel_id: &ChannelId,
    ) -> Result<Signer, TokenTransferError> {
        todo!()
    }

    fn _is_send_enabled(&self) -> bool {
        self.is_send_enabled
    }

    fn _is_receive_enabled(&self) -> bool {
        self.is_receive_enabled
    }
}
