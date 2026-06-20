use pinocchio::{
    AccountView,
    Address,
    entrypoint,
    error::ProgramError,
    ProgramResult,
};

pub const ID: Address = Address::new_from_array([
    0x12, 0x8c, 0xfa, 0x23, 0x1a, 0x34, 0x56, 0x78,
    0x9a, 0xbc, 0xde, 0xf0, 0x12, 0x34, 0x56, 0x78,
    0x9a, 0xbc, 0xde, 0xf0, 0x12, 0x34, 0x56, 0x78,
    0x9a, 0xbc, 0xde, 0xf0, 0x12, 0x34, 0x56, 0x78,
]);

mod state;
mod error;
mod instructions;

use instructions::*;

entrypoint!(process_instruction);

fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let (discriminator, data) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    match *discriminator {
        0 => initialize(data, accounts),
        1 => contribute(data, accounts),
        2 => check_contributions(data, accounts),
        3 => refund(data, accounts),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
