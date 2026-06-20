use pinocchio::{
    AccountView,
    Address,
    cpi::{Seed, Signer},
    error::ProgramError,
    ProgramResult,
};
use pinocchio_token::{
    instructions::Transfer,
    state::Account as TokenAccount,
};
use pinocchio_associated_token_account::instructions::CreateIdempotent;

use crate::{
    state::{Fundraiser, SEED_FUNDRAISER},
    error::{ERR_TARGET_NOT_MET, ERR_ARITHMETIC_OVERFLOW},
};
use crate::ID;

pub fn check_contributions(_data: &[u8], accounts: &mut [AccountView]) -> ProgramResult {
    let [maker, mint_to_raise, fundraiser, vault, maker_ata, token_program, system_program, associated_token_program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !maker.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let fund_data = fundraiser.try_borrow()?;
    let fund = Fundraiser::from_bytes(&fund_data)?;

    if fund.maker != maker.address().to_bytes() {
        return Err(ProgramError::InvalidAccountData);
    }

    let expected_fundraiser =
        Address::find_program_address(&[SEED_FUNDRAISER, fund.maker.as_ref()], &ID).0;
    if fundraiser.address() != &expected_fundraiser {
        return Err(ProgramError::InvalidSeeds);
    }

    let fund_bump = fund.bump;
    let fund_maker = fund.maker;
    let amount_to_raise = fund.amount_to_raise;
    drop(fund_data);

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

    let maker_ata_expected = Address::find_program_address(
        &[
            maker.address().as_ref(),
            token_program.address().as_ref(),
            mint_to_raise.address().as_ref(),
        ],
        &pinocchio_associated_token_account::ID,
    )
    .0;

    if maker_ata.address() != &maker_ata_expected {
        return Err(ProgramError::InvalidSeeds);
    }

    if !maker_ata.owned_by(&pinocchio_token::ID) {
        CreateIdempotent {
            funding_account: maker,
            account: maker_ata,
            wallet: maker,
            mint: mint_to_raise,
            system_program,
            token_program,
        }
        .invoke()?;
    }

    let vault_account = unsafe { TokenAccount::from_account_view_unchecked(vault)? };
    let vault_amount = vault_account.amount();

    if vault_amount < amount_to_raise {
        return Err(ProgramError::Custom(ERR_TARGET_NOT_MET));
    }

    let bump_byte = [fund_bump];
    let seeds = [
        Seed::from(SEED_FUNDRAISER),
        Seed::from(fund_maker.as_ref()),
        Seed::from(&bump_byte),
    ];
    let signers = [Signer::from(&seeds)];

    Transfer::new(vault, maker_ata, fundraiser, vault_amount)
        .invoke_signed(&signers)?;

    let dest_lamports = maker.lamports();
    maker.set_lamports(
        dest_lamports
            .checked_add(fundraiser.lamports())
            .ok_or(ProgramError::Custom(ERR_ARITHMETIC_OVERFLOW))?,
    )?;

    fundraiser.close()?;

    Ok(())
}
