use {
    crate::ibc_instruction::IbcInstruction,
    borsh::{BorshDeserialize, BorshSerialize},
    eclipse_ibc_known_proto::KnownProto,
    ibc_proto::google::protobuf,
    solana_program_runtime::{ic_msg, invoke_context::InvokeContext},
    solana_sdk::{
        instruction::InstructionError,
        transaction_context::{IndexOfAccount, InstructionContext, TransactionContext},
    },
};

#[derive(BorshDeserialize, BorshSerialize, Clone, Debug)]
pub struct IbcContractInstruction {
    pub extra_accounts_for_instruction: IndexOfAccount,
    pub last_instruction_part: Vec<u8>,
}

pub fn parse_instruction(
    invoke_context: &InvokeContext,
    transaction_context: &TransactionContext,
    instruction_context: &InstructionContext,
) -> Result<(IbcInstruction, IndexOfAccount), InstructionError> {
    let instruction_data = instruction_context.get_instruction_data();
    let IbcContractInstruction {
        extra_accounts_for_instruction,
        mut last_instruction_part,
    } = BorshDeserialize::try_from_slice(instruction_data).map_err(|err| {
        ic_msg!(
            invoke_context,
            "could not parse instruction as IbcContractInstruction: {:?}",
            err
        );
        InstructionError::InvalidInstructionData
    })?;

    let mut ibc_instruction_data: Vec<u8> = vec![];
    for account_index in 0..extra_accounts_for_instruction {
        let extra_account = instruction_context
            .try_borrow_instruction_account(transaction_context, account_index)?;
        ibc_instruction_data.extend_from_slice(extra_account.get_data());
    }

    ibc_instruction_data.append(&mut last_instruction_part);

    let any_msg = protobuf::Any::decode(&*ibc_instruction_data).map_err(|err| {
        ic_msg!(
            invoke_context,
            "could not parse instruction as Any Protobuf: {:?}",
            err
        );
        InstructionError::InvalidInstructionData
    })?;

    let type_url = any_msg.type_url.clone();
    ic_msg!(invoke_context, "IBC instruction type: {}", type_url);

    let ibc_instruction: IbcInstruction = any_msg.try_into().map_err(|err| {
        ic_msg!(
            invoke_context,
            "could not parse Any Protobuf into a specific instruction: {:?}",
            err
        );
        InstructionError::InvalidInstructionData
    })?;

    Ok((ibc_instruction, extra_accounts_for_instruction))
}
