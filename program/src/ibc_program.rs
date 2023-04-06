use {
    crate::{
        ibc_handler::IbcHandler,
        ibc_instruction::{
            msgs::{MsgBindPort, MsgReleasePort},
            IbcInstruction, PortInstruction,
        },
        id,
    },
    eclipse_ibc_state::IbcAccountData,
    ibc::core::handler::dispatch,
    ibc_proto::google::protobuf,
    known_proto::KnownProto,
    solana_program_runtime::{ic_msg, invoke_context::InvokeContext},
    solana_sdk::{instruction::InstructionError, transaction_context::IndexOfAccount},
};

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

    let mut ibc_account =
        instruction_context.try_borrow_instruction_account(transaction_context, 1)?;
    if *ibc_account.get_owner() != id() {
        return Err(InstructionError::InvalidAccountOwner);
    }

    let sysvar_cache = invoke_context.get_sysvar_cache();
    let clock = sysvar_cache.get_clock()?;
    let slot_hashes = sysvar_cache.get_slot_hashes()?;

    let mut ibc_account_data = IbcAccountData::read_from_account(&ibc_account, invoke_context)?;
    let mut ibc_handler = IbcHandler::new(
        &ibc_account_data.store,
        &mut ibc_account_data.metadata,
        &clock,
        slot_hashes,
    )
    .map_err(|err| {
        ic_msg!(invoke_context, "failed to init IBC handler: {:?}", err);
        InstructionError::InvalidInstructionData
    })?;

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

    match ibc_instruction {
        IbcInstruction::Router(envelope) => {
            dispatch(&mut ibc_handler, envelope).map_err(|err| {
                ic_msg!(invoke_context, "{} failed: {:?}", type_url, err);
                InstructionError::InvalidInstructionData
            })?;
        }
        IbcInstruction::Port(instruction) => {
            match instruction {
                PortInstruction::Bind(MsgBindPort { port_id }) => {
                    ibc_handler.bind_port(&port_id, calling_account.get_key())
                }
                PortInstruction::Release(MsgReleasePort { port_id }) => {
                    ibc_handler.release_port(&port_id, calling_account.get_key())
                }
            }
            .map_err(|err| {
                ic_msg!(invoke_context, "{} failed: {:?}", type_url, err);
                InstructionError::InvalidInstructionData
            })?;
        }
    }

    ibc_handler.commit().map_err(|err| {
        ic_msg!(
            invoke_context,
            "failed to commit the new IBC state Merkle tree: {:?}",
            err
        );
        InstructionError::InvalidAccountData
    })?;

    ibc_account_data.write_to_account(&mut ibc_account, invoke_context)?;

    Ok(())
}
