use pinocchio::{
    AccountView,
    Address,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::clock::Clock,
    ProgramResult,
};
use pinocchio_token::{
    instructions::Transfer,
    state::Account as TokenAccount,
};

use crate::{
    state::{
        Fundraiser, Contributor,
        SEED_FUNDRAISER, DISCRIMINATOR_CONTRIBUTOR,
    },
    error::{
        ERR_TARGET_MET, ERR_FUNDRAISER_NOT_ENDED,
        ERR_ARITHMETIC_OVERFLOW,
    },
};
use crate::ID;

const SECONDS_TO_DAYS: i64 = 86400;

pub fn refund(_data: &[u8], accounts: &mut [AccountView]) -> ProgramResult {
    let [contributor, maker, mint_to_raise, fundraiser, contributor_account, contributor_ata, vault, token_program, system_program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !contributor.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

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

    let current_time = Clock::get()?.unix_timestamp;
    let elapsed_days = current_time
        .checked_sub(fund.time_started)
        .ok_or(ProgramError::Custom(ERR_FUNDRAISER_NOT_ENDED))?
        .checked_div(SECONDS_TO_DAYS)
        .ok_or(ProgramError::Custom(ERR_FUNDRAISER_NOT_ENDED))?;

    if fund.duration > elapsed_days as u8 {
        return Err(ProgramError::Custom(ERR_FUNDRAISER_NOT_ENDED));
    }

    let fund_bump = fund.bump;
    let fund_maker = fund.maker;
    let amount_to_raise = fund.amount_to_raise;
    drop(fund_data);

    let vault_account = unsafe { TokenAccount::from_account_view_unchecked(vault)? };
    let vault_amount = vault_account.amount();

    if vault_amount >= amount_to_raise {
        return Err(ProgramError::Custom(ERR_TARGET_MET));
    }

    let expected_contributor_account = Contributor::derive_pda(
        fundraiser.address(),
        contributor.address(),
        &ID,
    )
    .0;
    if contributor_account.address() != &expected_contributor_account {
        return Err(ProgramError::InvalidSeeds);
    }

    let cont_data = contributor_account.try_borrow()?;
    if cont_data.len() < Contributor::LEN || cont_data[0] != DISCRIMINATOR_CONTRIBUTOR {
        return Err(ProgramError::InvalidAccountData);
    }
    let cont = Contributor::from_bytes(&cont_data)?;
    let refund_amount = cont.amount;
    drop(cont_data);

    if refund_amount == 0 {
        return Err(ProgramError::InvalidAccountData);
    }

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

    let bump_byte = [fund_bump];
    let seeds = [
        Seed::from(SEED_FUNDRAISER),
        Seed::from(fund_maker.as_ref()),
        Seed::from(&bump_byte),
    ];
    let signers = [Signer::from(&seeds)];

    Transfer::new(vault, contributor_ata, fundraiser, refund_amount)
        .invoke_signed(&signers)?;

    let mut fund_data = fundraiser.try_borrow_mut()?;
    let fund_mut = Fundraiser::from_bytes_mut(&mut fund_data)?;
    fund_mut.current_amount = fund_mut
        .current_amount
        .checked_sub(refund_amount)
        .ok_or(ProgramError::Custom(ERR_ARITHMETIC_OVERFLOW))?;
    drop(fund_data);

    let dest_lamports = contributor.lamports();
    contributor.set_lamports(
        dest_lamports
            .checked_add(contributor_account.lamports())
            .ok_or(ProgramError::Custom(ERR_ARITHMETIC_OVERFLOW))?,
    )?;

    contributor_account.close()?;

    Ok(())
}
