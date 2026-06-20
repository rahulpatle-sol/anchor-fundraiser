use pinocchio::{
    AccountView,
    Address,
    error::ProgramError,
    sysvars::{rent::Rent, clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_associated_token_account::instructions::Create as CreateATA;
use pinocchio_token::state::Mint;

use crate::{
    state::{Fundraiser},
    error::ERR_INVALID_AMOUNT,
};
use crate::ID;

const MIN_AMOUNT_TO_RAISE: u64 = 3;

pub fn initialize(data: &[u8], accounts: &mut [AccountView]) -> ProgramResult {
    let [maker, mint_to_raise, fundraiser, vault, token_program, associated_token_program, system_program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !maker.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let amount = u64::from_le_bytes(
        data.get(..8)
            .and_then(|b| b.try_into().ok())
            .ok_or(ProgramError::InvalidInstructionData)?,
    );
    let duration = *data.get(8).ok_or(ProgramError::InvalidInstructionData)?;

    let mint = unsafe { Mint::from_account_view_unchecked(mint_to_raise)? };
    let min_amount = MIN_AMOUNT_TO_RAISE
        .checked_pow(mint.decimals() as u32)
        .ok_or(ProgramError::Custom(ERR_INVALID_AMOUNT))?;

    if amount <= min_amount {
        return Err(ProgramError::Custom(ERR_INVALID_AMOUNT));
    }

    let (expected_fundraiser, canonical_bump) =
        Fundraiser::derive_pda(maker.address(), &ID);
    if fundraiser.address() != &expected_fundraiser {
        return Err(ProgramError::InvalidSeeds);
    }

    let vault_expected = Address::find_program_address(
        &[
            fundraiser.address().as_ref(),
            token_program.address().as_ref(),
            mint_to_raise.address().as_ref(),
        ],
        &pinocchio_associated_token_account::ID,
    )
    .0;
    if vault.address() != &vault_expected {
        return Err(ProgramError::InvalidSeeds);
    }

    let time_started = Clock::get()?.unix_timestamp;

    let fund_state = Fundraiser {
        time_started,
        amount_to_raise: amount,
        current_amount: 0,
        maker: maker.address().to_bytes(),
        mint_to_raise: mint_to_raise.address().to_bytes(),
        bump: canonical_bump,
        duration,
    };

    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(Fundraiser::LEN as u64);

    CreateAccount {
        from: maker,
        to: fundraiser,
        lamports,
        space: Fundraiser::LEN as u64,
        owner: &ID,
    }
    .invoke()?;

    let mut fund_data = fundraiser.try_borrow_mut()?;
    Fundraiser::init(&mut fund_data, &fund_state);
    drop(fund_data);

    CreateATA {
        funding_account: maker,
        account: vault,
        wallet: fundraiser,
        mint: mint_to_raise,
        system_program,
        token_program,
    }
    .invoke()?;

    Ok(())
}
