use {
    crate::{
        ibc_contract_instruction,
        ibc_handler::IbcHandler,
        ibc_instruction::{
            msgs::{
                MsgBindPort, MsgInitStorageAccount, MsgReleasePort, MsgWriteTxBuffer,
                MsgWriteTxBufferMode,
            },
            AdminInstruction, IbcInstruction, PortInstruction,
        },
        id,
    },
    eclipse_ibc_state::{internal_path::StateInitializedPath, IbcAccountData, IbcState},
    ibc::core::dispatch,
    solana_program_runtime::{
        ic_msg, invoke_context::InvokeContext, sysvar_cache::get_sysvar_with_account_check,
    },
    solana_sdk::{
        instruction::InstructionError,
        pubkey::Pubkey,
        syscalls::MAX_CPI_INSTRUCTION_DATA_LEN,
        system_instruction,
        transaction_context::{InstructionContext, TransactionContext},
    },
};

const ROUTER_ERR_CODE: u32 = 0x97;
const PORT_ERR_CODE: u32 = 0x98;
const STORAGE_ERR_CODE: u32 = 0x99;

pub const STORAGE_KEY: Pubkey = Pubkey::new_from_array([
    135, 90, 195, 29, 90, 182, 162, 153, 214, 170, 125, 126, 161, 2, 167, 102, 196, 107, 28, 247,
    252, 46, 240, 250, 117, 230, 224, 243, 31, 221, 167, 136,
]);

fn with_ibc_handler<F>(
    invoke_context: &InvokeContext,
    transaction_context: &TransactionContext,
    instruction_context: &InstructionContext,
    account_offset: usize,
    f: F,
) -> Result<(), InstructionError>
where
    F: FnOnce(&mut IbcHandler) -> Result<(), InstructionError>,
{
    instruction_context.check_number_of_instruction_accounts(account_offset + 3)?;

    let mut storage_account = instruction_context
        .try_borrow_instruction_account(transaction_context, account_offset + 1)?;
    if *storage_account.get_owner() != id() {
        return Err(InstructionError::InvalidAccountOwner);
    }
    if *storage_account.get_key() != STORAGE_KEY {
        return Err(InstructionError::InvalidArgument);
    }

    let clock = get_sysvar_with_account_check::clock(
        invoke_context,
        instruction_context,
        account_offset + 2,
    )?;

    let mut ibc_account_data = IbcAccountData::read_from_account(&storage_account, invoke_context)?;
    let mut ibc_handler = IbcHandler::new(
        &ibc_account_data.store,
        &mut ibc_account_data.metadata,
        &clock,
    )
    .map_err(|err| {
        ic_msg!(invoke_context, "failed to init IBC handler: {:?}", err);
        InstructionError::InvalidAccountData
    })?;

    f(&mut ibc_handler)?;

    ibc_handler.commit().map_err(|err| {
        ic_msg!(
            invoke_context,
            "failed to commit the new IBC state Merkle tree: {:?}",
            err
        );
        InstructionError::Custom(STORAGE_ERR_CODE)
    })?;

    ibc_account_data.write_to_account(&mut storage_account, invoke_context)?;
    Ok(())
}

fn init_storage_account(
    invoke_context: &mut InvokeContext,
    account_offset: usize,
    payer_key: Pubkey,
    min_rent_balance: u64,
) -> Result<(), InstructionError> {
    // System account is at index 4
    invoke_context.native_invoke(
        system_instruction::create_account(
            &payer_key,
            &STORAGE_KEY,
            min_rent_balance,
            MAX_CPI_INSTRUCTION_DATA_LEN,
            &id(),
        ),
        &[STORAGE_KEY],
    )?;

    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;

    let mut storage_account = instruction_context
        .try_borrow_instruction_account(transaction_context, account_offset + 1)?;
    if *storage_account.get_key() != STORAGE_KEY {
        return Err(InstructionError::InvalidArgument);
    }

    let clock = get_sysvar_with_account_check::clock(
        invoke_context,
        instruction_context,
        account_offset + 3,
    )?;

    let ibc_account_data = IbcAccountData::default();

    let mut ibc_state = IbcState::new(&ibc_account_data.store, clock.slot);
    ibc_state.set(&StateInitializedPath, ());
    ibc_state.commit().map_err(|err| {
        ic_msg!(
            invoke_context,
            "failed to commit the new IBC state Merkle tree: {:?}",
            err
        );
        InstructionError::Custom(STORAGE_ERR_CODE)
    })?;

    ibc_account_data.write_to_account(&mut storage_account, invoke_context)?;
    Ok(())
}

fn create_tx_buffer(
    invoke_context: &mut InvokeContext,
    buffer_key: Pubkey,
    payer_key: Pubkey,
    min_rent_balance: u64,
    buffer_size: u64,
) -> Result<(), InstructionError> {
    // System account is at index 3
    invoke_context.native_invoke(
        system_instruction::create_account(
            &payer_key,
            &buffer_key,
            min_rent_balance,
            buffer_size,
            &id(),
        ),
        &[buffer_key],
    )?;
    Ok(())
}

fn write_to_tx_buffer(
    invoke_context: &mut InvokeContext,
    account_offset: usize,
    data_offset: usize,
    data: &[u8],
) -> Result<(), InstructionError> {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;

    let mut buffer_account = instruction_context
        .try_borrow_instruction_account(transaction_context, account_offset + 1)?;
    let buffer = &mut buffer_account.get_data_mut()?[data_offset..data_offset + data.len()];

    if buffer.len() != data.len() {
        ic_msg!(
            invoke_context,
            "buffer account does not fit entirety of data",
        );
        return Err(InstructionError::Custom(STORAGE_ERR_CODE));
    }

    buffer.copy_from_slice(data);

    Ok(())
}

/// # Errors
/// Returns an error if processing the instruction fails due to any of the
/// errors listed in `InstructionError`.
pub fn process_instruction(
    _first_instruction_account: usize,
    invoke_context: &mut InvokeContext,
) -> Result<(), InstructionError> {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;

    let (ibc_instruction, account_offset) = ibc_contract_instruction::parse_instruction(
        invoke_context,
        transaction_context,
        instruction_context,
    )?;

    let payer_account =
        instruction_context.try_borrow_instruction_account(transaction_context, account_offset)?;
    if !payer_account.is_signer() {
        return Err(InstructionError::MissingRequiredSignature);
    }
    let payer_key = *payer_account.get_key();

    match ibc_instruction {
        IbcInstruction::Router(envelope) => {
            with_ibc_handler(
                invoke_context,
                transaction_context,
                instruction_context,
                account_offset,
                |ibc_handler| {
                    dispatch(ibc_handler, envelope).map_err(|err| {
                        ic_msg!(invoke_context, "instruction failed: {:?}", err);
                        InstructionError::Custom(ROUTER_ERR_CODE)
                    })
                },
            )?;
        }
        IbcInstruction::Port(PortInstruction::Bind(MsgBindPort { port_id })) => {
            with_ibc_handler(
                invoke_context,
                transaction_context,
                instruction_context,
                account_offset,
                |ibc_handler| {
                    ibc_handler.bind_port(&port_id, &payer_key).map_err(|err| {
                        ic_msg!(invoke_context, "instruction failed: {:?}", err);
                        InstructionError::Custom(PORT_ERR_CODE)
                    })
                },
            )?;
        }
        IbcInstruction::Port(PortInstruction::Release(MsgReleasePort { port_id })) => {
            with_ibc_handler(
                invoke_context,
                transaction_context,
                instruction_context,
                account_offset,
                |ibc_handler| {
                    ibc_handler
                        .release_port(&port_id, &payer_key)
                        .map_err(|err| {
                            ic_msg!(invoke_context, "instruction failed: {:?}", err);
                            InstructionError::Custom(PORT_ERR_CODE)
                        })
                },
            )?;
        }
        IbcInstruction::Admin(AdminInstruction::InitStorageAccount(MsgInitStorageAccount)) => {
            instruction_context.check_number_of_instruction_accounts(account_offset + 5)?;

            let rent = get_sysvar_with_account_check::rent(
                invoke_context,
                instruction_context,
                account_offset + 2,
            )?;
            let min_rent_balance = rent.minimum_balance(MAX_CPI_INSTRUCTION_DATA_LEN as usize);

            // Accounts need to be dropped because `invoke_context.native_invoke`
            // requires `&mut invoke_context`.
            drop(payer_account);

            init_storage_account(invoke_context, account_offset, payer_key, min_rent_balance)?;
        }
        IbcInstruction::Admin(AdminInstruction::WriteTxBuffer(MsgWriteTxBuffer { mode, data })) => {
            // Accounts need to be dropped because `invoke_context.native_invoke`
            // requires `&mut invoke_context`.
            drop(payer_account);

            let data_offset = match mode {
                MsgWriteTxBufferMode::Create { buffer_size } => {
                    instruction_context.check_number_of_instruction_accounts(account_offset + 4)?;

                    let buffer_key =
                        *transaction_context.get_key_of_account_at_index(account_offset + 1)?;

                    let rent = get_sysvar_with_account_check::rent(
                        invoke_context,
                        instruction_context,
                        account_offset + 2,
                    )?;
                    let min_rent_balance = rent.minimum_balance(buffer_size as usize);

                    create_tx_buffer(
                        invoke_context,
                        buffer_key,
                        payer_key,
                        min_rent_balance,
                        buffer_size,
                    )?;

                    0
                }
                MsgWriteTxBufferMode::Reuse { offset } => {
                    instruction_context.check_number_of_instruction_accounts(account_offset + 2)?;

                    offset
                }
            };

            write_to_tx_buffer(invoke_context, account_offset, data_offset as usize, &data)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VAULT_SEED: &[u8] = b"eclipse-ibc";
    const BUMP_SEED: u8 = 254;

    #[test]
    fn storage_key_is_pda() {
        let (expected_pda, bump_seed) = Pubkey::find_program_address(&[VAULT_SEED], &id());
        assert_eq!(
            expected_pda.to_string(),
            "A7NJxtiKpEFL4TSTygkKSkf5b2g719DJbvQPRr4moUHD",
        );
        assert_eq!(expected_pda, STORAGE_KEY);
        assert_eq!(bump_seed, BUMP_SEED);
        assert!(!expected_pda.is_on_curve());

        let actual_pda =
            Pubkey::create_program_address(&[VAULT_SEED, &[BUMP_SEED]], &id()).unwrap();
        assert_eq!(expected_pda, actual_pda);
    }
}
