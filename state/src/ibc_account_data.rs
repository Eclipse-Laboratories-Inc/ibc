use {
    crate::{IbcMetadata, IbcStore},
    core::fmt::Debug,
    serde::{Deserialize, Serialize},
    solana_program_runtime::{ic_msg, invoke_context::InvokeContext},
    solana_sdk::{instruction::InstructionError, transaction_context::BorrowedAccount},
};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct IbcAccountData {
    pub store: IbcStore,
    pub metadata: IbcMetadata,
}

impl IbcAccountData {
    pub fn read_from_account(
        account: &BorrowedAccount<'_>,
        invoke_context: &InvokeContext,
    ) -> Result<Self, InstructionError> {
        let account_data = account.get_data();
        bincode::deserialize::<Self>(account_data).map_err(|err| {
            ic_msg!(
                invoke_context,
                "failed to deserialize IBC account data: {:?}",
                err,
            );
            InstructionError::InvalidAccountData
        })
    }

    pub fn write_to_account(
        &self,
        account: &mut BorrowedAccount<'_>,
        invoke_context: &InvokeContext,
    ) -> Result<(), InstructionError> {
        let account_data = bincode::serialize(&self).map_err(|err| {
            ic_msg!(
                invoke_context,
                "failed to serialize new IBC account data: {:?}",
                err,
            );
            InstructionError::InvalidAccountData
        })?;
        account.set_data(account_data)?;
        Ok(())
    }
}
