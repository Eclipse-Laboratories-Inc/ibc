mod ibc_handler;
pub mod ibc_instruction;
mod ibc_program;
mod ics20_module;
pub mod module_instruction;

solana_sdk::declare_id!("Ec11pse1bc111111111111111111111111111111111");

pub use ibc_program::process_instruction;
