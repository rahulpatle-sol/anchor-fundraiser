use pinocchio::{
    AccountView,
    Address,
    error::ProgramError,
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::{instructions::Transfer, state::Mint};

use crate::{
    state::{
        Fundraiser, Contributor,
        SEED_FUNDRAISER, DISCRIMINATOR_CONTRIBUTOR,
    },
    error::{
        ERR_CONTRIBUTION_TOO_SMALL, ERR_CONTRIBUTION_TOO_BIG,
        ERR_MAXIMUM_CONTRIBUTIONS_REACHED, ERR_FUNDRAISER_ENDED,
        ERR_ARITHMETIC_OVERFLOW,
    },
};
use crate::ID;

const SECONDS_TO_DAYS: i64 = 86400;
const MAX_CONTRIBUTION_PERCENTAGE: u64 = 10;
const PERCENTAGE_SCALER: u64 = 100;

pub fn contribute(data: &[u8], accounts: &mut [AccountView]) -> ProgramResult {
    let [contributor, mint_to_raise, fundraiser, contributor_account, contributor_ata, vault, token_program, system_program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !contributor.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let amount = u64::from_le_bytes(
        data.get(..8)
            .and_then(|b| b.try_into().ok())
            .ok_or(ProgramError::InvalidInstructionData)?,
    );

    let fund_data = fundraiser.try_borrow()?;
    let fund = Fundraiser::from_bytes(&fund_data)?;

    if fund.mint_to_raise != mint_to_raise.address().to_bytes() {
        return Err(ProgramError::InvalidAccountData);
    }

    let expected_fundraiser =
        Address::find_program_address(&[SEED_FUNDRAISER, fund.maker.as_ref()], &ID).0;
    if fundraiser.address() != &expected_fundraiser {
        return Err(ProgramError::InvalidSeeds);
    }

    let mint = unsafe { Mint::from_account_view_unchecked(mint_to_raise)? };
    let min_contribution = 1u64
        .checked_pow(mint.decimals() as u32)
        .ok_or(ProgramError::Custom(ERR_CONTRIBUTION_TOO_SMALL))?;

    if amount <= min_contribution {
        return Err(ProgramError::Custom(ERR_CONTRIBUTION_TOO_SMALL));
    }

    let max_allowed = fund
        .amount_to_raise
        .checked_mul(MAX_CONTRIBUTION_PERCENTAGE)
        .ok_or(ProgramError::Custom(ERR_ARITHMETIC_OVERFLOW))?
        .checked_div(PERCENTAGE_SCALER)
        .ok_or(ProgramError::Custom(ERR_ARITHMETIC_OVERFLOW))?;

    if amount > max_allowed {
        return Err(ProgramError::Custom(ERR_CONTRIBUTION_TOO_BIG));
    }

    let current_time = Clock::get()?.unix_timestamp;
    let elapsed_days = current_time
        .checked_sub(fund.time_started)
        .ok_or(ProgramError::Custom(ERR_FUNDRAISER_ENDED))?
        .checked_div(SECONDS_TO_DAYS)
        .ok_or(ProgramError::Custom(ERR_FUNDRAISER_ENDED))?;

    if fund.duration <= elapsed_days as u8 {
        return Err(ProgramError::Custom(ERR_FUNDRAISER_ENDED));
    }
    let fund_maker = fund.maker;
    let fund_amount_to_raise = fund.amount_to_raise;
    drop(fund_data);

    let expected_contributor = Contributor::derive_pda(
        fundraiser.address(),
        contributor.address(),
        &ID,
    )
    .0;

    let has_existing_contributor = if contributor_account.data_len() == 0 {
        false
    } else {
        let cont_data = contributor_account.try_borrow()?;
        cont_data.len() >= Contributor::LEN && cont_data[0] == DISCRIMINATOR_CONTRIBUTOR
    };

    if !has_existing_contributor {
        if contributor_account.address() != &expected_contributor {
            return Err(ProgramError::InvalidSeeds);
        }
        if !contributor_account.owned_by(&pinocchio_system::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(Contributor::LEN as u64);

        CreateAccount {
            from: contributor,
            to: contributor_account,
            lamports,
            space: Contributor::LEN as u64,
            owner: &ID,
        }
        .invoke()?;

        let mut cont_data = contributor_account.try_borrow_mut()?;
        Contributor::init(&mut cont_data, &Contributor { amount: 0 });
    } else {
        if contributor_account.address() != &expected_contributor {
            return Err(ProgramError::InvalidSeeds);
        }
    }

    let cont_data = contributor_account.try_borrow()?;
    let cont = Contributor::from_bytes(&cont_data)?;

    if cont.amount > max_allowed {
        return Err(ProgramError::Custom(ERR_MAXIMUM_CONTRIBUTIONS_REACHED));
    }
    let new_total = cont
        .amount
        .checked_add(amount)
        .ok_or(ProgramError::Custom(ERR_ARITHMETIC_OVERFLOW))?;
    if new_total > max_allowed {
        return Err(ProgramError::Custom(ERR_MAXIMUM_CONTRIBUTIONS_REACHED));
    }
    let cont_current = cont.amount;
    drop(cont_data);

    let contributor_ata_expected = Address::find_program_address(
        &[
            contributor.address().as_ref(),
            token_program.address().as_ref(),
            mint_to_raise.address().as_ref(),
        ],
        &pinocchio_associated_token_account::ID,
    )
    .0;
    if contributor_ata.address() != &contributor_ata_expected {
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

    Transfer::new(contributor_ata, vault, contributor, amount).invoke()?;

    let mut fund_data = fundraiser.try_borrow_mut()?;
    let fund = Fundraiser::from_bytes_mut(&mut fund_data)?;
    fund.current_amount = fund
        .current_amount
        .checked_add(amount)
        .ok_or(ProgramError::Custom(ERR_ARITHMETIC_OVERFLOW))?;
    drop(fund_data);

    let mut cont_data = contributor_account.try_borrow_mut()?;
    let cont = Contributor::from_bytes_mut(&mut cont_data)?;
    cont.amount = cont_current
        .checked_add(amount)
        .ok_or(ProgramError::Custom(ERR_ARITHMETIC_OVERFLOW))?;

    Ok(())
}
