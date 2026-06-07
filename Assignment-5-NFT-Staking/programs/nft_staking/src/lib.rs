use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Mint, MintTo, Token, TokenAccount, Transfer},
};

declare_id!("6uyxk3pVKKVf7sCdqApLwhWC4KG9rk2w8HU9VrLQRXHv");

#[program]
pub mod nft_staking {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        reward_rate_per_second: u64,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.reward_mint = ctx.accounts.reward_mint.key();
        config.collection_mint = ctx.accounts.collection_mint.key();
        config.reward_rate_per_second = reward_rate_per_second;
        config.bump = ctx.bumps.config;
        config.reward_authority_bump = ctx.bumps.reward_mint_authority;

        let plugin = &mut ctx.accounts.collection_plugin;
        plugin.collection_mint = ctx.accounts.collection_mint.key();
        plugin.bump = ctx.bumps.collection_plugin;
        plugin.attributes = vec![Attribute {
            key: "staked_nfts".to_string(),
            value: "0".to_string(),
        }];

        Ok(())
    }

    pub fn stake_nft(ctx: Context<StakeNft>) -> Result<()> {
        let stake_record = &mut ctx.accounts.stake_record;
        require!(!stake_record.active, StakingError::AlreadyStaked);

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.staker_nft_ata.to_account_info(),
                    to: ctx.accounts.vault_nft_ata.to_account_info(),
                    authority: ctx.accounts.staker.to_account_info(),
                },
            ),
            1,
        )?;

        let now = Clock::get()?.unix_timestamp;
        stake_record.staker = ctx.accounts.staker.key();
        stake_record.nft_mint = ctx.accounts.nft_mint.key();
        stake_record.staked_at = now;
        stake_record.last_claimed_at = now;
        stake_record.active = true;
        stake_record.bump = ctx.bumps.stake_record;
        stake_record.vault_authority_bump = ctx.bumps.vault_authority;

        increment_staked_nft_count(&mut ctx.accounts.collection_plugin)?;

        Ok(())
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let stake_record = &mut ctx.accounts.stake_record;

        require!(stake_record.active, StakingError::StakeInactive);
        require!(now >= stake_record.last_claimed_at, StakingError::InvalidTimestamp);

        let elapsed = now
            .checked_sub(stake_record.last_claimed_at)
            .ok_or(StakingError::MathOverflow)? as u64;

        let amount = elapsed
            .checked_mul(ctx.accounts.config.reward_rate_per_second)
            .ok_or(StakingError::MathOverflow)?;

        if amount > 0 {
            let signer_seeds: &[&[&[u8]]] = &[&[
                b"reward-authority",
                ctx.accounts.config.to_account_info().key.as_ref(),
                &[ctx.accounts.config.reward_authority_bump],
            ]];

            token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    MintTo {
                        mint: ctx.accounts.reward_mint.to_account_info(),
                        to: ctx.accounts.staker_reward_ata.to_account_info(),
                        authority: ctx.accounts.reward_mint_authority.to_account_info(),
                    },
                    signer_seeds,
                ),
                amount,
            )?;
        }

        stake_record.last_claimed_at = now;

        Ok(())
    }

    pub fn unstake_nft(ctx: Context<UnstakeNft>) -> Result<()> {
        let stake_record = &mut ctx.accounts.stake_record;
        require!(stake_record.active, StakingError::StakeInactive);
        let stake_record_key = stake_record.key();

        let signer_seeds: &[&[&[u8]]] = &[&[
            b"vault",
            stake_record_key.as_ref(),
            &[stake_record.vault_authority_bump],
        ]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_nft_ata.to_account_info(),
                    to: ctx.accounts.staker_nft_ata.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                signer_seeds,
            ),
            1,
        )?;

        stake_record.active = false;

        decrement_staked_nft_count(&mut ctx.accounts.collection_plugin)?;

        Ok(())
    }
}

fn increment_staked_nft_count(plugin: &mut Account<CollectionAttributesPlugin>) -> Result<()> {
    let current = read_staked_count(plugin)?;
    let next = current.checked_add(1).ok_or(StakingError::MathOverflow)?;
    write_staked_count(plugin, next)
}

fn decrement_staked_nft_count(plugin: &mut Account<CollectionAttributesPlugin>) -> Result<()> {
    let current = read_staked_count(plugin)?;
    let next = current.checked_sub(1).ok_or(StakingError::MathOverflow)?;
    write_staked_count(plugin, next)
}

fn read_staked_count(plugin: &CollectionAttributesPlugin) -> Result<u64> {
    plugin
        .attributes
        .iter()
        .find(|entry| entry.key == "staked_nfts")
        .ok_or(StakingError::MissingCollectionAttribute)?
        .value
        .parse::<u64>()
        .map_err(|_| StakingError::InvalidAttributeValue.into())
}

fn write_staked_count(plugin: &mut CollectionAttributesPlugin, count: u64) -> Result<()> {
    let value = count.to_string();
    if let Some(entry) = plugin
        .attributes
        .iter_mut()
        .find(|entry| entry.key == "staked_nfts")
    {
        entry.value = value;
    } else {
        plugin.attributes.push(Attribute {
            key: "staked_nfts".to_string(),
            value,
        });
    }

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + Config::INIT_SPACE,
        seeds = [b"config", authority.key().as_ref()],
        bump
    )]
    pub config: Account<'info, Config>,

    pub collection_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        space = 8 + CollectionAttributesPlugin::INIT_SPACE,
        seeds = [b"collection-plugin", collection_mint.key().as_ref()],
        bump
    )]
    pub collection_plugin: Account<'info, CollectionAttributesPlugin>,

    #[account(mut)]
    pub reward_mint: Account<'info, Mint>,

    /// CHECK: PDA only used as mint authority signer.
    #[account(
        seeds = [b"reward-authority", config.key().as_ref()],
        bump
    )]
    pub reward_mint_authority: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StakeNft<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(mut)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [b"collection-plugin", config.collection_mint.as_ref()],
        bump = collection_plugin.bump
    )]
    pub collection_plugin: Box<Account<'info, CollectionAttributesPlugin>>,

    pub nft_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = staker
    )]
    pub staker_nft_ata: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = staker,
        associated_token::mint = nft_mint,
        associated_token::authority = vault_authority
    )]
    pub vault_nft_ata: Box<Account<'info, TokenAccount>>,

    /// CHECK: PDA signer for the vault ATA.
    #[account(
        seeds = [b"vault", stake_record.key().as_ref()],
        bump
    )]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = staker,
        space = 8 + StakeRecord::INIT_SPACE,
        seeds = [b"stake", staker.key().as_ref(), nft_mint.key().as_ref()],
        bump
    )]
    pub stake_record: Box<Account<'info, StakeRecord>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(mut)]
    pub config: Account<'info, Config>,

    pub nft_mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [b"stake", staker.key().as_ref(), nft_mint.key().as_ref()],
        bump = stake_record.bump,
        has_one = staker,
        has_one = nft_mint
    )]
    pub stake_record: Account<'info, StakeRecord>,

    #[account(mut, address = config.reward_mint)]
    pub reward_mint: Account<'info, Mint>,

    /// CHECK: PDA signer for reward mint authority.
    #[account(
        seeds = [b"reward-authority", config.key().as_ref()],
        bump = config.reward_authority_bump
    )]
    pub reward_mint_authority: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = staker,
        associated_token::mint = reward_mint,
        associated_token::authority = staker
    )]
    pub staker_reward_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UnstakeNft<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    pub config: Box<Account<'info, Config>>,

    pub nft_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        seeds = [b"stake", staker.key().as_ref(), nft_mint.key().as_ref()],
        bump = stake_record.bump,
        has_one = staker,
        has_one = nft_mint
    )]
    pub stake_record: Box<Account<'info, StakeRecord>>,

    #[account(
        mut,
        seeds = [b"collection-plugin", config.collection_mint.as_ref()],
        bump = collection_plugin.bump
    )]
    pub collection_plugin: Box<Account<'info, CollectionAttributesPlugin>>,

    #[account(
        init_if_needed,
        payer = staker,
        associated_token::mint = nft_mint,
        associated_token::authority = staker
    )]
    pub staker_nft_ata: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = vault_authority
    )]
    pub vault_nft_ata: Box<Account<'info, TokenAccount>>,

    /// CHECK: PDA signer for vault ATA.
    #[account(
        seeds = [b"vault", stake_record.key().as_ref()],
        bump = stake_record.vault_authority_bump
    )]
    pub vault_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub authority: Pubkey,
    pub reward_mint: Pubkey,
    pub collection_mint: Pubkey,
    pub reward_rate_per_second: u64,
    pub bump: u8,
    pub reward_authority_bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct Attribute {
    #[max_len(32)]
    pub key: String,
    #[max_len(32)]
    pub value: String,
}

#[account]
#[derive(InitSpace)]
pub struct CollectionAttributesPlugin {
    pub collection_mint: Pubkey,
    #[max_len(8)]
    pub attributes: Vec<Attribute>,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct StakeRecord {
    pub staker: Pubkey,
    pub nft_mint: Pubkey,
    pub staked_at: i64,
    pub last_claimed_at: i64,
    pub active: bool,
    pub bump: u8,
    pub vault_authority_bump: u8,
}

#[error_code]
pub enum StakingError {
    #[msg("NFT is already staked")]
    AlreadyStaked,
    #[msg("Stake record is inactive")]
    StakeInactive,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Invalid timestamp")]
    InvalidTimestamp,
    #[msg("Collection attribute is missing")]
    MissingCollectionAttribute,
    #[msg("Collection attribute value is invalid")]
    InvalidAttributeValue,
}
