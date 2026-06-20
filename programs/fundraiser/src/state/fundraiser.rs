use core::mem::size_of;
use pinocchio::{
    Address,
    error::ProgramError,
};

pub const DISCRIMINATOR_FUNDRAISER: u8 = 0;
pub const VERSION: u8 = 0;
pub const SEED_FUNDRAISER: &[u8] = b"fundraiser";

#[repr(C)]
pub struct Fundraiser {
    pub time_started: i64,
    pub amount_to_raise: u64,
    pub current_amount: u64,
    pub maker: [u8; 32],
    pub mint_to_raise: [u8; 32],
    pub bump: u8,
    pub duration: u8,
}

impl Fundraiser {
    pub const DATA_LEN: usize = size_of::<Fundraiser>();
    pub const LEN: usize = 2 + Self::DATA_LEN;

    pub fn from_bytes(data: &[u8]) -> Result<&Self, ProgramError> {
        if data.len() < Self::LEN || data[0] != DISCRIMINATOR_FUNDRAISER {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &*(data[2..].as_ptr() as *const Self) })
    }

    pub fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self, ProgramError> {
        if data.len() < Self::LEN || data[0] != DISCRIMINATOR_FUNDRAISER {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *(data[2..].as_mut_ptr() as *mut Self) })
    }

    pub fn init(data: &mut [u8], val: &Self) {
        data[0] = DISCRIMINATOR_FUNDRAISER;
        data[1] = VERSION;
        unsafe { (data[2..].as_mut_ptr() as *mut Self).write(*val) }
    }

    pub fn derive_pda(maker: &Address, program_id: &Address) -> (Address, u8) {
        Address::find_program_address(&[SEED_FUNDRAISER, maker.as_ref()], program_id)
    }

    pub fn update_current_amount(&mut self, amount: u64) -> Result<(), ProgramError> {
        self.current_amount = self
            .current_amount
            .checked_add(amount)
            .ok_or(ProgramError::Custom(crate::error::ERR_ARITHMETIC_OVERFLOW))?;
        Ok(())
    }

    pub fn sub_current_amount(&mut self, amount: u64) -> Result<(), ProgramError> {
        self.current_amount = self
            .current_amount
            .checked_sub(amount)
            .ok_or(ProgramError::Custom(crate::error::ERR_ARITHMETIC_OVERFLOW))?;
        Ok(())
    }
}
