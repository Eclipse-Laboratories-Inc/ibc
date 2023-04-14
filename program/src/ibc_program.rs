use {
    crate::{
        ibc_handler::IbcHandler,
        ibc_instruction::{
            msgs::{MsgBindPort, MsgInitStorageAccount, MsgReleasePort},
            AdminInstruction, IbcInstruction, PortInstruction,
        },
        id,
    },
    eclipse_ibc_state::IbcAccountData,
    ibc::core::handler::dispatch,
    ibc_proto::google::protobuf,
    known_proto::KnownProto,
    solana_program_runtime::{
        ic_msg, invoke_context::InvokeContext, sysvar_cache::get_sysvar_with_account_check,
    },
    solana_sdk::{
        instruction::InstructionError,
        pubkey::Pubkey,
        syscalls::MAX_CPI_INSTRUCTION_DATA_LEN,
        system_instruction,
        transaction_context::IndexOfAccount,
        transaction_context::{InstructionContext, TransactionContext},
    },
};

const ROUTER_ERR_CODE: u32 = 151;
const PORT_ERR_CODE: u32 = 152;

const VAULT_SEEDS: &[&[u8]] = &[b"eclipse-ibc"];

fn with_ibc_handler<F>(
    invoke_context: &InvokeContext,
    transaction_context: &TransactionContext,
    instruction_context: &InstructionContext,
    expected_storage_key: Pubkey,
    f: F,
) -> Result<(), InstructionError>
where
    F: FnOnce(&mut IbcHandler) -> Result<(), InstructionError>,
{
    let mut storage_account =
        instruction_context.try_borrow_instruction_account(transaction_context, 1)?;
    if *storage_account.get_owner() != id() {
        return Err(InstructionError::InvalidAccountOwner);
    }
    if *storage_account.get_key() != expected_storage_key {
        return Err(InstructionError::InvalidArgument);
    }

    let clock = get_sysvar_with_account_check::clock(invoke_context, instruction_context, 2)?;
    let slot_hashes =
        get_sysvar_with_account_check::slot_hashes(invoke_context, instruction_context, 3)?;

    let mut ibc_account_data = IbcAccountData::read_from_account(&storage_account, invoke_context)?;
    let mut ibc_handler = IbcHandler::new(
        &ibc_account_data.store,
        &mut ibc_account_data.metadata,
        &clock,
        slot_hashes,
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
        InstructionError::InvalidAccountData
    })?;

    ibc_account_data.write_to_account(&mut storage_account, invoke_context)?;
    Ok(())
}

fn init_storage_account(
    invoke_context: &mut InvokeContext,
    expected_storage_key: Pubkey,
    calling_key: Pubkey,
    min_rent_balance: u64,
) -> Result<(), InstructionError> {
    invoke_context.native_invoke(
        system_instruction::create_account(
            &calling_key,
            &expected_storage_key,
            min_rent_balance,
            MAX_CPI_INSTRUCTION_DATA_LEN,
            &id(),
        ),
        &[calling_key, expected_storage_key],
    )?;

    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;

    let mut storage_account =
        instruction_context.try_borrow_instruction_account(transaction_context, 1)?;
    if *storage_account.get_key() != expected_storage_key {
        return Err(InstructionError::InvalidArgument);
    }

    let ibc_account_data = IbcAccountData::default();
    ibc_account_data.write_to_account(&mut storage_account, invoke_context)?;
    Ok(())
}

/// # Errors
/// Returns an error if processing the instruction fails due to any of the
/// errors listed in `InstructionError`.
pub fn process_instruction(
    _first_instruction_account: IndexOfAccount,
    invoke_context: &mut InvokeContext,
) -> Result<(), InstructionError> {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;

    let calling_account =
        instruction_context.try_borrow_instruction_account(transaction_context, 0)?;
    if !calling_account.is_signer() {
        return Err(InstructionError::MissingRequiredSignature);
    }

    let instruction_data = instruction_context.get_instruction_data();
    let any_msg = protobuf::Any::decode(instruction_data).map_err(|err| {
        ic_msg!(
            invoke_context,
            "could not parse instruction as Any Protobuf: {:?}",
            err
        );
        InstructionError::InvalidInstructionData
    })?;

    let type_url = any_msg.type_url.clone();
    ic_msg!(invoke_context, &type_url);

    let ibc_instruction: IbcInstruction = any_msg.try_into().map_err(|err| {
        ic_msg!(
            invoke_context,
            "could not parse Any Protobuf into a specific instruction: {:?}",
            err
        );
        InstructionError::InvalidInstructionData
    })?;

    let expected_storage_key = Pubkey::create_program_address(VAULT_SEEDS, &id())?;

    match ibc_instruction {
        IbcInstruction::Router(envelope) => with_ibc_handler(
            invoke_context,
            transaction_context,
            instruction_context,
            expected_storage_key,
            |ibc_handler| {
                dispatch(ibc_handler, envelope).map_err(|err| {
                    ic_msg!(invoke_context, "{} failed: {:?}", type_url, err);
                    InstructionError::Custom(ROUTER_ERR_CODE)
                })
            },
        )?,
        IbcInstruction::Port(instruction) => with_ibc_handler(
            invoke_context,
            transaction_context,
            instruction_context,
            expected_storage_key,
            |ibc_handler| match instruction {
                PortInstruction::Bind(MsgBindPort { port_id }) => {
                    ibc_handler
                        .bind_port(&port_id, calling_account.get_key())
                        .map_err(|err| {
                            ic_msg!(invoke_context, "{} failed: {:?}", type_url, err);
                            InstructionError::Custom(PORT_ERR_CODE)
                        })?;
                    Ok(())
                }
                PortInstruction::Release(MsgReleasePort { port_id }) => {
                    ibc_handler
                        .release_port(&port_id, calling_account.get_key())
                        .map_err(|err| {
                            ic_msg!(invoke_context, "{} failed: {:?}", type_url, err);
                            InstructionError::Custom(PORT_ERR_CODE)
                        })?;
                    Ok(())
                }
            },
        )?,
        IbcInstruction::Admin(instruction) => {
            match instruction {
                AdminInstruction::InitStorageAccount(MsgInitStorageAccount) => {
                    let calling_key = *calling_account.get_key();
                    let rent = get_sysvar_with_account_check::rent(
                        invoke_context,
                        instruction_context,
                        2,
                    )?;
                    let min_rent_balance =
                        rent.minimum_balance(MAX_CPI_INSTRUCTION_DATA_LEN as usize);

                    // Accounts need to be dropped because `invoke_context.native_invoke`
                    // requires `&mut invoke_context`.
                    drop(calling_account);

                    init_storage_account(
                        invoke_context,
                        expected_storage_key,
                        calling_key,
                        min_rent_balance,
                    )?;
                }
            }
        }
    }

    Ok(())
}
