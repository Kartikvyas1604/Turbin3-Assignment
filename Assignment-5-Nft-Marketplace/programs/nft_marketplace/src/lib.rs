use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, TokenAccount};
use anchor_spl::token_interface::{
    transfer_checked, TokenInterface, TransferChecked,
};

declare_id!("7WHNLLRezRc2GZD1QdFaBWHEpb6kuTqZggLsWXUAFVcc");

#[program]
pub mod nft_marketplace {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, fee_bps: u16) -> Result<()> {
        require!(!ctx.accounts.config.is_initialized, MarketplaceError::AlreadyInitialized);
        let config = &mut ctx.accounts.config;
        config.admin = ctx.accounts.admin.key();
        config.fee_bps = fee_bps;
        config.treasury = ctx.accounts.treasury.key();
        config.rewards_mint = ctx.accounts.rewards_mint.key();
        config.is_initialized = true;
        Ok(())
    }

    pub fn list(ctx: Context<List>, price: u64) -> Result<()> {
        require!(price > 0, MarketplaceError::InvalidPrice);

        let listing = &mut ctx.accounts.listing;
        listing.maker = ctx.accounts.maker.key();
        listing.asset_mint = ctx.accounts.asset_mint.key();
        listing.price = price;
        listing.payment_mint = Pubkey::default();
        listing.bump = ctx.bumps.listing;

        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.maker_ata.to_account_info(),
                    mint: ctx.accounts.asset_mint.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.maker.to_account_info(),
                },
            ),
            1,
            ctx.accounts.asset_mint.decimals,
        )?;

        Ok(())
    }

    pub fn list_with_token(ctx: Context<ListWithToken>, price: u64) -> Result<()> {
        require!(price > 0, MarketplaceError::InvalidPrice);

        let listing = &mut ctx.accounts.listing;
        listing.maker = ctx.accounts.maker.key();
        listing.asset_mint = ctx.accounts.asset_mint.key();
        listing.price = price;
        listing.payment_mint = ctx.accounts.payment_mint.key();
        listing.bump = ctx.bumps.listing;

        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.maker_ata.to_account_info(),
                    mint: ctx.accounts.asset_mint.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.maker.to_account_info(),
                },
            ),
            1,
            ctx.accounts.asset_mint.decimals,
        )?;

        Ok(())
    }

    pub fn delist(ctx: Context<Delist>) -> Result<()> {
        let listing = &ctx.accounts.listing;
        let seeds = &[
            b"listing".as_ref(),
            listing.asset_mint.as_ref(),
            listing.maker.as_ref(),
            &[listing.bump],
        ];
        let signer_seeds = &[&seeds[..]];

        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault.to_account_info(),
                    mint: ctx.accounts.asset_mint.to_account_info(),
                    to: ctx.accounts.maker_ata.to_account_info(),
                    authority: ctx.accounts.listing.to_account_info(),
                },
                signer_seeds,
            ),
            1,
            ctx.accounts.asset_mint.decimals,
        )?;

        Ok(())
    }

    pub fn buy(ctx: Context<Buy>) -> Result<()> {
        let listing = &ctx.accounts.listing;
        require!(
            listing.payment_mint == Pubkey::default(),
            MarketplaceError::WrongPaymentMethod
        );

        let config = &ctx.accounts.config;
        let fee_amount = (listing.price as u128)
            .checked_mul(config.fee_bps as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap() as u64;
        let maker_amount = listing.price - fee_amount;

        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.buyer.key(),
                &listing.maker,
                maker_amount,
            ),
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.maker.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        if fee_amount > 0 {
            anchor_lang::solana_program::program::invoke(
                &anchor_lang::solana_program::system_instruction::transfer(
                    &ctx.accounts.buyer.key(),
                    &config.treasury,
                    fee_amount,
                ),
                &[
                    ctx.accounts.buyer.to_account_info(),
                    ctx.accounts.treasury.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
            )?;
        }

        let seeds = &[
            b"listing".as_ref(),
            listing.asset_mint.as_ref(),
            listing.maker.as_ref(),
            &[listing.bump],
        ];
        let signer_seeds = &[&seeds[..]];
        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault.to_account_info(),
                    mint: ctx.accounts.asset_mint.to_account_info(),
                    to: ctx.accounts.buyer_ata.to_account_info(),
                    authority: ctx.accounts.listing.to_account_info(),
                },
                signer_seeds,
            ),
            1,
            ctx.accounts.asset_mint.decimals,
        )?;

        Ok(())
    }

    pub fn buy_with_token(ctx: Context<BuyWithToken>) -> Result<()> {
        let listing = &ctx.accounts.listing;
        require!(
            listing.payment_mint == ctx.accounts.payment_mint.key(),
            MarketplaceError::WrongPaymentMethod
        );

        let config = &ctx.accounts.config;
        let fee_amount = (listing.price as u128)
            .checked_mul(config.fee_bps as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap() as u64;
        let maker_amount = listing.price - fee_amount;

        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.buyer_payment_ata.to_account_info(),
                    mint: ctx.accounts.payment_mint.to_account_info(),
                    to: ctx.accounts.maker_payment_ata.to_account_info(),
                    authority: ctx.accounts.buyer.to_account_info(),
                },
            ),
            maker_amount,
            ctx.accounts.payment_mint.decimals,
        )?;

        if fee_amount > 0 {
            transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.buyer_payment_ata.to_account_info(),
                        mint: ctx.accounts.payment_mint.to_account_info(),
                        to: ctx.accounts.treasury_payment_ata.to_account_info(),
                        authority: ctx.accounts.buyer.to_account_info(),
                    },
                ),
                fee_amount,
                ctx.accounts.payment_mint.decimals,
            )?;
        }

        let seeds = &[
            b"listing".as_ref(),
            listing.asset_mint.as_ref(),
            listing.maker.as_ref(),
            &[listing.bump],
        ];
        let signer_seeds = &[&seeds[..]];
        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault.to_account_info(),
                    mint: ctx.accounts.asset_mint.to_account_info(),
                    to: ctx.accounts.buyer_ata.to_account_info(),
                    authority: ctx.accounts.listing.to_account_info(),
                },
                signer_seeds,
            ),
            1,
            ctx.accounts.asset_mint.decimals,
        )?;

        Ok(())
    }

    pub fn make_offer(ctx: Context<MakeOffer>, amount: u64) -> Result<()> {
        require!(amount > 0, MarketplaceError::InvalidPrice);

        let offer = &mut ctx.accounts.offer;
        offer.buyer = ctx.accounts.buyer.key();
        offer.asset_mint = ctx.accounts.asset_mint.key();
        offer.amount = amount;
        offer.bump = ctx.bumps.offer;

        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.buyer.key(),
                &ctx.accounts.offer.key(),
                amount,
            ),
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.offer.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        Ok(())
    }

    pub fn accept_offer(ctx: Context<AcceptOffer>) -> Result<()> {
        let offer = &ctx.accounts.offer;
        let config = &ctx.accounts.config;
        let fee_amount = (offer.amount as u128)
            .checked_mul(config.fee_bps as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap() as u64;
        let maker_amount = offer.amount - fee_amount;

        let offer_info = ctx.accounts.offer.to_account_info();

        // Transfer the NFT via token program CPI
        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.asset_owner_ata.to_account_info(),
                    mint: ctx.accounts.asset_mint.to_account_info(),
                    to: ctx.accounts.buyer_ata.to_account_info(),
                    authority: ctx.accounts.asset_owner.to_account_info(),
                },
            ),
            1,
            ctx.accounts.asset_mint.decimals,
        )?;

        // Handle SOL transfers via direct lamport manipulation
        let asset_owner_info = ctx.accounts.asset_owner.to_account_info();
        let treasury_info = ctx.accounts.treasury.to_account_info();
        let buyer_info = ctx.accounts.buyer.to_account_info();

        let offer_lamports = offer_info.lamports();
        let asset_owner_lamports = asset_owner_info.lamports();
        let treasury_lamports = treasury_info.lamports();
        let buyer_lamports = buyer_info.lamports();

        // Transfer maker_amount from offer to asset_owner
        **offer_info.lamports.borrow_mut() = offer_lamports
            .checked_sub(maker_amount)
            .unwrap();
        **asset_owner_info.lamports.borrow_mut() = asset_owner_lamports
            .checked_add(maker_amount)
            .unwrap();

        // Transfer fee from offer to treasury
        let after_maker = offer_info.lamports();
        **treasury_info.lamports.borrow_mut() = treasury_lamports
            .checked_add(fee_amount)
            .unwrap();
        **offer_info.lamports.borrow_mut() = after_maker
            .checked_sub(fee_amount)
            .unwrap();

        // Transfer remaining rent from offer to buyer
        let rent = offer_info.lamports();
        **buyer_info.lamports.borrow_mut() = buyer_lamports
            .checked_add(rent)
            .unwrap();
        **offer_info.lamports.borrow_mut() = 0;

        Ok(())
    }

    pub fn cancel_offer(_ctx: Context<CancelOffer>) -> Result<()> {
        Ok(())
    }
}

#[account]
pub struct Config {
    pub admin: Pubkey,
    pub fee_bps: u16,
    pub treasury: Pubkey,
    pub rewards_mint: Pubkey,
    pub is_initialized: bool,
}
impl Config {
    pub const LEN: usize = 32 + 2 + 32 + 32 + 1;
}

#[account]
pub struct Listing {
    pub maker: Pubkey,
    pub asset_mint: Pubkey,
    pub price: u64,
    pub payment_mint: Pubkey,
    pub bump: u8,
}
impl Listing {
    pub const LEN: usize = 32 + 32 + 8 + 32 + 1;
}

#[account]
pub struct Offer {
    pub buyer: Pubkey,
    pub asset_mint: Pubkey,
    pub amount: u64,
    pub bump: u8,
}
impl Offer {
    pub const LEN: usize = 32 + 32 + 8 + 1;
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + Config::LEN,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut)]
    pub treasury: SystemAccount<'info>,
    pub rewards_mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct List<'info> {
    #[account(
        init,
        payer = maker,
        space = 8 + Listing::LEN,
        seeds = [b"listing", asset_mint.key().as_ref(), maker.key().as_ref()],
        bump
    )]
    pub listing: Account<'info, Listing>,
    #[account(mut)]
    pub maker: Signer<'info>,
    pub asset_mint: Account<'info, Mint>,
    #[account(
        mut,
        token::mint = asset_mint,
        token::authority = maker,
    )]
    pub maker_ata: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = maker,
        associated_token::mint = asset_mint,
        associated_token::authority = listing,
    )]
    pub vault: Account<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ListWithToken<'info> {
    #[account(
        init,
        payer = maker,
        space = 8 + Listing::LEN,
        seeds = [b"listing", asset_mint.key().as_ref(), maker.key().as_ref()],
        bump
    )]
    pub listing: Account<'info, Listing>,
    #[account(mut)]
    pub maker: Signer<'info>,
    pub asset_mint: Account<'info, Mint>,
    pub payment_mint: Account<'info, Mint>,
    #[account(
        mut,
        token::mint = asset_mint,
        token::authority = maker,
    )]
    pub maker_ata: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = maker,
        associated_token::mint = asset_mint,
        associated_token::authority = listing,
    )]
    pub vault: Account<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Delist<'info> {
    #[account(
        mut,
        close = maker,
        seeds = [b"listing", asset_mint.key().as_ref(), maker.key().as_ref()],
        bump
    )]
    pub listing: Account<'info, Listing>,
    #[account(mut)]
    pub maker: Signer<'info>,
    pub asset_mint: Account<'info, Mint>,
    #[account(
        mut,
        token::mint = asset_mint,
        token::authority = maker,
    )]
    pub maker_ata: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = asset_mint,
        associated_token::authority = listing,
    )]
    pub vault: Account<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Buy<'info> {
    #[account(
        mut,
        close = maker,
        seeds = [b"listing", asset_mint.key().as_ref(), maker.key().as_ref()],
        bump
    )]
    pub listing: Account<'info, Listing>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    #[account(mut)]
    pub maker: SystemAccount<'info>,
    pub asset_mint: Account<'info, Mint>,
    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = asset_mint,
        associated_token::authority = buyer,
    )]
    pub buyer_ata: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = asset_mint,
        associated_token::authority = listing,
    )]
    pub vault: Account<'info, TokenAccount>,
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub treasury: SystemAccount<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BuyWithToken<'info> {
    #[account(
        mut,
        close = maker,
        seeds = [b"listing", asset_mint.key().as_ref(), maker.key().as_ref()],
        bump
    )]
    pub listing: Account<'info, Listing>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    #[account(mut)]
    pub maker: SystemAccount<'info>,
    pub asset_mint: Account<'info, Mint>,
    pub payment_mint: Account<'info, Mint>,
    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = asset_mint,
        associated_token::authority = buyer,
    )]
    pub buyer_ata: Account<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = payment_mint,
        token::authority = buyer,
    )]
    pub buyer_payment_ata: Box<Account<'info, TokenAccount>>,
    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = payment_mint,
        associated_token::authority = maker,
    )]
    pub maker_payment_ata: Box<Account<'info, TokenAccount>>,
    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = payment_mint,
        associated_token::authority = treasury,
    )]
    pub treasury_payment_ata: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = asset_mint,
        associated_token::authority = listing,
    )]
    pub vault: Box<Account<'info, TokenAccount>>,
    pub config: Account<'info, Config>,
    pub treasury: SystemAccount<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MakeOffer<'info> {
    #[account(
        init,
        payer = buyer,
        space = 8 + Offer::LEN,
        seeds = [b"offer", asset_mint.key().as_ref(), buyer.key().as_ref()],
        bump
    )]
    pub offer: Account<'info, Offer>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    pub asset_mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AcceptOffer<'info> {
    #[account(
        mut,
        seeds = [b"offer", asset_mint.key().as_ref(), buyer.key().as_ref()],
        bump
    )]
    pub offer: Account<'info, Offer>,
    #[account(mut)]
    pub buyer: SystemAccount<'info>,
    #[account(mut)]
    pub asset_owner: Signer<'info>,
    pub asset_mint: Account<'info, Mint>,
    #[account(
        mut,
        token::mint = asset_mint,
        token::authority = asset_owner,
    )]
    pub asset_owner_ata: Account<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = asset_mint,
        token::authority = buyer,
    )]
    pub buyer_ata: Account<'info, TokenAccount>,
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub treasury: SystemAccount<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelOffer<'info> {
    #[account(
        mut,
        close = buyer,
        seeds = [b"offer", asset_mint.key().as_ref(), buyer.key().as_ref()],
        bump
    )]
    pub offer: Account<'info, Offer>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    pub asset_mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum MarketplaceError {
    #[msg("Already initialized")]
    AlreadyInitialized,
    #[msg("Price must be greater than 0")]
    InvalidPrice,
    #[msg("Wrong payment method for this listing")]
    WrongPaymentMethod,
}
