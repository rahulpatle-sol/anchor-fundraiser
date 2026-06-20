use core::mem::size_of;
use pinocchio::{
    address::Address,
    error::ProgramError,
};

pub const DISCRIMINATOR_CONTRIBUTOR: u8 = 1;
pub const SEED_CONTRIBUTOR: &[u8] = b"contributor";

#[repr(C)]
pub struct Contributor {
    pub amount: u64,
}

impl Contributor {
    pub const DATA_LEN: usize = size_of::<Contributor>();
    pub const LEN: usize = 2 + Self::DATA_LEN;

    pub fn from_bytes(data: &[u8]) -> Result<&Self, ProgramError> {
        if data.len() < Self::LEN || data[0] != DISCRIMINATOR_CONTRIBUTOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &*(data[2..].as_ptr() as *const Self) })
    }

    pub fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self, ProgramError> {
        if data.len() < Self::LEN || data[0] != DISCRIMINATOR_CONTRIBUTOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *(data[2..].as_mut_ptr() as *mut Self) })
    }

    pub fn init(data: &mut [u8], val: &Self) {
        data[0] = DISCRIMINATOR_CONTRIBUTOR;
        data[1] = 0;
        unsafe { (data[2..].as_mut_ptr() as *mut Self).write(*val) }
    }

    pub fn derive_pda(
        fundraiser: &Address,
        contributor: &Address,
        program_id: &Address,
    ) -> (Address, u8) {
        Address::find_program_address(
            &[SEED_CONTRIBUTOR, fundraiser.as_ref(), contributor.as_ref()],
            program_id,
        )
    }
}
