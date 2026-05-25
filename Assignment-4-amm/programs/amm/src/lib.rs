use anchor_lang::prelude::*;
use anchor_lang::system_program;

declare_id!("CJZU4fsC4LZGkCyRw3AoMKM8y1GQWGCmnuu5kTBbMece");

#[program]
pub mod amm {
    use super::*;

    pub fn initialize_pool(ctx: Context<InitializePool>) -> Result<()> {
        let pool = &mut ctx.accounts.pool;

        pool.authority = ctx.accounts.authority.key();
        pool.vault_a = ctx.accounts.vault_a.key();
        pool.vault_b = ctx.accounts.vault_b.key();
        pool.reserve_a = 0;
        pool.reserve_b = 0;
        pool.total_lp = 0;
        pool.bump = ctx.bumps.pool;

        Ok(())
    }

    pub fn add_liquidity(ctx: Context<AddLiquidity>, amount_a: u64, amount_b: u64) -> Result<()> {
        require!(amount_a > 0, AmmError::InvalidAmount);
        require!(amount_b > 0, AmmError::InvalidAmount);

        let pool = &mut ctx.accounts.pool;
        let lp_account = &mut ctx.accounts.lp_account;

        let lp_minted = if pool.total_lp == 0 {
            amount_a
                .checked_add(amount_b)
                .ok_or(AmmError::MathOverflow)?
        } else {
            let left = (amount_a as u128)
                .checked_mul(pool.reserve_b as u128)
                .ok_or(AmmError::MathOverflow)?;
            let right = (amount_b as u128)
                .checked_mul(pool.reserve_a as u128)
                .ok_or(AmmError::MathOverflow)?;
            require!(left == right, AmmError::InvalidRatio);

            let numerator = (amount_a as u128)
                .checked_mul(pool.total_lp as u128)
                .ok_or(AmmError::MathOverflow)?;
            let lp = numerator / (pool.reserve_a as u128);
            require!(lp > 0, AmmError::InvalidAmount);
            u64::try_from(lp).map_err(|_| AmmError::MathOverflow)?
        };

        let cpi_accounts_a = system_program::Transfer {
            from: ctx.accounts.user.to_account_info(),
            to: ctx.accounts.vault_a.to_account_info(),
        };
        let cpi_accounts_b = system_program::Transfer {
            from: ctx.accounts.user.to_account_info(),
            to: ctx.accounts.vault_b.to_account_info(),
        };

        let cpi_program = ctx.accounts.system_program.to_account_info();
        system_program::transfer(CpiContext::new(cpi_program.clone(), cpi_accounts_a), amount_a)?;
        system_program::transfer(CpiContext::new(cpi_program, cpi_accounts_b), amount_b)?;

        pool.reserve_a = pool
            .reserve_a
            .checked_add(amount_a)
            .ok_or(AmmError::MathOverflow)?;
        pool.reserve_b = pool
            .reserve_b
            .checked_add(amount_b)
            .ok_or(AmmError::MathOverflow)?;
        pool.total_lp = pool
            .total_lp
            .checked_add(lp_minted)
            .ok_or(AmmError::MathOverflow)?;

        lp_account.owner = ctx.accounts.user.key();
        lp_account.pool = pool.key();
        lp_account.bump = ctx.bumps.lp_account;
        lp_account.amount = lp_account
            .amount
            .checked_add(lp_minted)
            .ok_or(AmmError::MathOverflow)?;

        Ok(())
    }

    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, lp_amount: u64) -> Result<()> {
        require!(lp_amount > 0, AmmError::InvalidAmount);

        let pool = &mut ctx.accounts.pool;
        let lp_account = &mut ctx.accounts.lp_account;

        require!(lp_account.owner == ctx.accounts.user.key(), AmmError::InvalidOwner);
        require!(lp_account.pool == pool.key(), AmmError::InvalidPool);
        require!(lp_account.amount >= lp_amount, AmmError::NotEnoughLiquidity);
        require!(pool.total_lp >= lp_amount, AmmError::NotEnoughLiquidity);

        let amount_a = (pool.reserve_a as u128)
            .checked_mul(lp_amount as u128)
            .ok_or(AmmError::MathOverflow)?
            / (pool.total_lp as u128);
        let amount_b = (pool.reserve_b as u128)
            .checked_mul(lp_amount as u128)
            .ok_or(AmmError::MathOverflow)?
            / (pool.total_lp as u128);

        let amount_a = u64::try_from(amount_a).map_err(|_| AmmError::MathOverflow)?;
        let amount_b = u64::try_from(amount_b).map_err(|_| AmmError::MathOverflow)?;

        require!(amount_a > 0, AmmError::InvalidAmount);
        require!(amount_b > 0, AmmError::InvalidAmount);

        let vault_a_balance = ctx.accounts.vault_a.to_account_info().lamports();
        let vault_b_balance = ctx.accounts.vault_b.to_account_info().lamports();
        require!(vault_a_balance >= amount_a, AmmError::NotEnoughLiquidity);
        require!(vault_b_balance >= amount_b, AmmError::NotEnoughLiquidity);

        pool.reserve_a = pool
            .reserve_a
            .checked_sub(amount_a)
            .ok_or(AmmError::MathOverflow)?;
        pool.reserve_b = pool
            .reserve_b
            .checked_sub(amount_b)
            .ok_or(AmmError::MathOverflow)?;
        pool.total_lp = pool
            .total_lp
            .checked_sub(lp_amount)
            .ok_or(AmmError::MathOverflow)?;

        lp_account.amount = lp_account
            .amount
            .checked_sub(lp_amount)
            .ok_or(AmmError::MathOverflow)?;

        **ctx.accounts.vault_a.to_account_info().try_borrow_mut_lamports()? -= amount_a;
        **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += amount_a;

        **ctx.accounts.vault_b.to_account_info().try_borrow_mut_lamports()? -= amount_b;
        **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += amount_b;

        Ok(())
    }

    pub fn swap(ctx: Context<Swap>, amount_in: u64, min_out: u64, a_to_b: bool) -> Result<()> {
        require!(amount_in > 0, AmmError::InvalidAmount);

        let pool = &mut ctx.accounts.pool;

        let (reserve_in, reserve_out) = if a_to_b {
            (pool.reserve_a, pool.reserve_b)
        } else {
            (pool.reserve_b, pool.reserve_a)
        };

        require!(reserve_in > 0, AmmError::NotEnoughLiquidity);
        require!(reserve_out > 0, AmmError::NotEnoughLiquidity);

        let amount_out = get_amount_out(amount_in, reserve_in, reserve_out)?;
        require!(amount_out >= min_out, AmmError::SlippageExceeded);
        require!(amount_out > 0, AmmError::InvalidAmount);

        if a_to_b {
            let cpi_accounts = system_program::Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: ctx.accounts.vault_a.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(ctx.accounts.system_program.to_account_info(), cpi_accounts);
            system_program::transfer(cpi_ctx, amount_in)?;

            let vault_b_balance = ctx.accounts.vault_b.to_account_info().lamports();
            require!(vault_b_balance >= amount_out, AmmError::NotEnoughLiquidity);

            **ctx.accounts.vault_b.to_account_info().try_borrow_mut_lamports()? -= amount_out;
            **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += amount_out;

            pool.reserve_a = pool
                .reserve_a
                .checked_add(amount_in)
                .ok_or(AmmError::MathOverflow)?;
            pool.reserve_b = pool
                .reserve_b
                .checked_sub(amount_out)
                .ok_or(AmmError::MathOverflow)?;
        } else {
            let cpi_accounts = system_program::Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: ctx.accounts.vault_b.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(ctx.accounts.system_program.to_account_info(), cpi_accounts);
            system_program::transfer(cpi_ctx, amount_in)?;

            let vault_a_balance = ctx.accounts.vault_a.to_account_info().lamports();
            require!(vault_a_balance >= amount_out, AmmError::NotEnoughLiquidity);

            **ctx.accounts.vault_a.to_account_info().try_borrow_mut_lamports()? -= amount_out;
            **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += amount_out;

            pool.reserve_b = pool
                .reserve_b
                .checked_add(amount_in)
                .ok_or(AmmError::MathOverflow)?;
            pool.reserve_a = pool
                .reserve_a
                .checked_sub(amount_out)
                .ok_or(AmmError::MathOverflow)?;
        }

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + PoolState::LEN,
        seeds = [b"pool", authority.key().as_ref()],
        bump
    )]
    pub pool: Account<'info, PoolState>,

    #[account(
        init,
        payer = authority,
        space = 8 + Vault::LEN,
        seeds = [b"vault_a", pool.key().as_ref()],
        bump
    )]
    pub vault_a: Account<'info, Vault>,

    #[account(
        init,
        payer = authority,
        space = 8 + Vault::LEN,
        seeds = [b"vault_b", pool.key().as_ref()],
        bump
    )]
    pub vault_b: Account<'info, Vault>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub pool: Account<'info, PoolState>,

    #[account(
        mut,
        seeds = [b"vault_a", pool.key().as_ref()],
        bump
    )]
    pub vault_a: Account<'info, Vault>,

    #[account(
        mut,
        seeds = [b"vault_b", pool.key().as_ref()],
        bump
    )]
    pub vault_b: Account<'info, Vault>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + LpAccount::LEN,
        seeds = [b"lp", pool.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub lp_account: Account<'info, LpAccount>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub pool: Account<'info, PoolState>,

    #[account(
        mut,
        seeds = [b"vault_a", pool.key().as_ref()],
        bump
    )]
    pub vault_a: Account<'info, Vault>,

    #[account(
        mut,
        seeds = [b"vault_b", pool.key().as_ref()],
        bump
    )]
    pub vault_b: Account<'info, Vault>,

    #[account(
        mut,
        seeds = [b"lp", pool.key().as_ref(), user.key().as_ref()],
        bump = lp_account.bump
    )]
    pub lp_account: Account<'info, LpAccount>,
}

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub pool: Account<'info, PoolState>,

    #[account(
        mut,
        seeds = [b"vault_a", pool.key().as_ref()],
        bump
    )]
    pub vault_a: Account<'info, Vault>,

    #[account(
        mut,
        seeds = [b"vault_b", pool.key().as_ref()],
        bump
    )]
    pub vault_b: Account<'info, Vault>,

    pub system_program: Program<'info, System>,
}

#[account]
pub struct PoolState {
    pub authority: Pubkey,
    pub vault_a: Pubkey,
    pub vault_b: Pubkey,
    pub reserve_a: u64,
    pub reserve_b: u64,
    pub total_lp: u64,
    pub bump: u8,
}

impl PoolState {
    pub const LEN: usize = 32 + 32 + 32 + 8 + 8 + 8 + 1;
}

#[account]
pub struct LpAccount {
    pub owner: Pubkey,
    pub pool: Pubkey,
    pub amount: u64,
    pub bump: u8,
}

impl LpAccount {
    pub const LEN: usize = 32 + 32 + 8 + 1;
}

#[account]
pub struct Vault {}

impl Vault {
    pub const LEN: usize = 0;
}

#[error_code]
pub enum AmmError {
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Not enough liquidity")]
    NotEnoughLiquidity,
    #[msg("Invalid ratio")]
    InvalidRatio,
    #[msg("Slippage exceeded")]
    SlippageExceeded,
    #[msg("Invalid owner")]
    InvalidOwner,
    #[msg("Invalid pool")]
    InvalidPool,
}

fn get_amount_out(amount_in: u64, reserve_in: u64, reserve_out: u64) -> Result<u64> {
    let amount_in = amount_in as u128;
    let reserve_in = reserve_in as u128;
    let reserve_out = reserve_out as u128;

    let amount_in_with_fee = amount_in
        .checked_mul(997)
        .ok_or(AmmError::MathOverflow)?
        / 1000;

    let numerator = amount_in_with_fee
        .checked_mul(reserve_out)
        .ok_or(AmmError::MathOverflow)?;
    let denominator = reserve_in
        .checked_add(amount_in_with_fee)
        .ok_or(AmmError::MathOverflow)?;

    let amount_out = numerator / denominator;
    let amount_out = u64::try_from(amount_out).map_err(|_| AmmError::MathOverflow)?;

    Ok(amount_out)
}
