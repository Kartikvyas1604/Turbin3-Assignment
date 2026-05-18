use anchor_lang::prelude::*;
use anchor_lang::system_program;

declare_id!("CVHSKiTKrivVkVy1Z8M2E1nBEKVddisehdPs5eW9qgKw");

#[program]
pub mod escrow {
    use super::*;

    pub fn make(ctx: Context<Make>, amount_a: u64, amount_b: u64) -> Result<()> {
        require!(amount_a > 0, EscrowError::InvalidAmount);
        require!(amount_b > 0, EscrowError::InvalidAmount);

        let escrow = &mut ctx.accounts.escrow;
        escrow.maker = ctx.accounts.maker.key();
        escrow.amount_a = amount_a;
        escrow.amount_b = amount_b;
        escrow.bump = ctx.bumps.escrow;

        let cpi_accounts = system_program::Transfer {
            from: ctx.accounts.maker.to_account_info(),
            to: ctx.accounts.escrow.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.to_account_info(), cpi_accounts);
        system_program::transfer(cpi_ctx, amount_a)?;

        Ok(())
    }

    pub fn take(ctx: Context<Take>) -> Result<()> {
        let escrow = &ctx.accounts.escrow;
        require!(escrow.amount_a > 0, EscrowError::InvalidAmount);
        require!(escrow.amount_b > 0, EscrowError::InvalidAmount);

        let escrow_lamports = ctx.accounts.escrow.to_account_info().lamports();
        require!(escrow_lamports >= escrow.amount_a, EscrowError::NotEnoughBalance);

        let cpi_accounts = system_program::Transfer {
            from: ctx.accounts.taker.to_account_info(),
            to: ctx.accounts.maker.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.to_account_info(), cpi_accounts);
        system_program::transfer(cpi_ctx, escrow.amount_b)?;

        **ctx.accounts.escrow.to_account_info().try_borrow_mut_lamports()? -= escrow.amount_a;
        **ctx.accounts.taker.to_account_info().try_borrow_mut_lamports()? += escrow.amount_a;

        Ok(())
    }

    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        let escrow = &ctx.accounts.escrow;
        require!(escrow.amount_a > 0, EscrowError::InvalidAmount);

        let escrow_lamports = ctx.accounts.escrow.to_account_info().lamports();
        require!(escrow_lamports >= escrow.amount_a, EscrowError::NotEnoughBalance);

        **ctx.accounts.escrow.to_account_info().try_borrow_mut_lamports()? -= escrow.amount_a;
        **ctx.accounts.maker.to_account_info().try_borrow_mut_lamports()? += escrow.amount_a;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Make<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    #[account(
        init,
        payer = maker,
        space = 8 + Escrow::LEN,
        seeds = [b"escrow", maker.key().as_ref()],
        bump
    )]
    pub escrow: Account<'info, Escrow>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Take<'info> {
    #[account(mut)]
    pub taker: Signer<'info>,

    #[account(mut, address = escrow.maker)]
    pub maker: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [b"escrow", maker.key().as_ref()],
        bump = escrow.bump,
        close = maker
    )]
    pub escrow: Account<'info, Escrow>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Refund<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    #[account(
        mut,
        seeds = [b"escrow", maker.key().as_ref()],
        bump = escrow.bump,
        close = maker
    )]
    pub escrow: Account<'info, Escrow>,
}

#[account]
pub struct Escrow {
    pub maker: Pubkey,
    pub amount_a: u64,
    pub amount_b: u64,
    pub bump: u8,
}

impl Escrow {
    pub const LEN: usize = 32 + 8 + 8 + 1;
}

#[error_code]
pub enum EscrowError {
    #[msg("Invalid amount")]
    InvalidAmount,

    #[msg("Not enough balance")]
    NotEnoughBalance,
}
