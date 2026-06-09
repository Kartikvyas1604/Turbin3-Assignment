use anchor_lang::prelude::*;
use solana_program::instruction::Instruction;
use solana_program::sysvar::instructions::load_instruction_at;

declare_id!("BQQBCRLNyrRKzQDHgZ1ogLAHA5YkmgeW7RDEF3Lx8YHe");

pub const SECP256K1_PROGRAM_ID: Pubkey =
    pubkey!("KeccakSecp256k11111111111111111111111111111");

#[error_code]
pub enum BetError {
    #[msg("Bet is already resolved")]
    AlreadyResolved,
    #[msg("Bet is not yet resolved")]
    NotResolved,
    #[msg("Invalid bet ID")]
    InvalidBetId,
    #[msg("Invalid winner: must be player A or B")]
    InvalidWinner,
    #[msg("Secp256k1 verification instruction not found in transaction")]
    Secp256k1InstructionNotFound,
    #[msg("Message in secp256k1 instruction does not match bet outcome")]
    MessageMismatch,
    #[msg("Oracle address in secp256k1 instruction does not match bet oracle")]
    OracleMismatch,
    #[msg("Failed to introspect instructions")]
    IntrospectionError,
    #[msg("Caller is not the winner")]
    NotWinner,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BetOutcome {
    pub bet_id: u64,
    pub winner: Pubkey,
}

const BET_SIZE: usize = 8     // discriminator
    + 8                       // bet_id
    + 32                      // player_a
    + 32                      // player_b
    + 8                       // amount
    + 20                      // oracle
    + 1                       // resolved
    + 32                      // winner
    + 1;                      // bump

#[account]
pub struct Bet {
    pub bet_id: u64,
    pub player_a: Pubkey,
    pub player_b: Pubkey,
    pub amount: u64,
    pub oracle: [u8; 20],
    pub resolved: bool,
    pub winner: Pubkey,
    pub bump: u8,
}

#[derive(Accounts)]
#[instruction(bet_id: u64)]
pub struct CreateBet<'info> {
    #[account(
        init,
        payer = player_a,
        space = BET_SIZE,
        seeds = [b"bet", bet_id.to_le_bytes().as_ref()],
        bump
    )]
    pub bet: Account<'info, Bet>,
    #[account(mut)]
    pub player_a: Signer<'info>,
    /// CHECK: Player B does not sign at creation
    pub player_b: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct JoinBet<'info> {
    #[account(mut)]
    pub bet: Account<'info, Bet>,
    #[account(mut)]
    pub player_b: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResolveBet<'info> {
    #[account(mut)]
    pub bet: Account<'info, Bet>,
    /// CHECK: Instructions sysvar for instruction introspection
    pub instructions: AccountInfo<'info>,
    pub caller: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClaimWinnings<'info> {
    #[account(mut, close = winner)]
    pub bet: Account<'info, Bet>,
    #[account(mut)]
    pub winner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[program]
pub mod betting {
    use super::*;

    pub fn create_bet(
        ctx: Context<CreateBet>,
        bet_id: u64,
        amount: u64,
        oracle: [u8; 20],
    ) -> Result<()> {
        let bet = &mut ctx.accounts.bet;
        bet.bet_id = bet_id;
        bet.player_a = ctx.accounts.player_a.key();
        bet.player_b = ctx.accounts.player_b.key();
        bet.amount = amount;
        bet.oracle = oracle;
        bet.resolved = false;
        bet.winner = Pubkey::default();
        bet.bump = ctx.bumps.bet;

        transfer_from(&ctx.accounts.player_a, &bet.to_account_info(), amount)
    }

    pub fn join_bet(ctx: Context<JoinBet>) -> Result<()> {
        let bet = &mut ctx.accounts.bet;
        require!(
            ctx.accounts.player_b.key() == bet.player_b,
            BetError::InvalidWinner
        );
        require!(!bet.resolved, BetError::AlreadyResolved);

        transfer_from(
            &ctx.accounts.player_b,
            &bet.to_account_info(),
            bet.amount,
        )
    }

    pub fn resolve_bet(
        ctx: Context<ResolveBet>,
        bet_outcome: BetOutcome,
    ) -> Result<()> {
        let bet = &mut ctx.accounts.bet;
        require!(!bet.resolved, BetError::AlreadyResolved);
        require!(bet_outcome.bet_id == bet.bet_id, BetError::InvalidBetId);
        require!(
            bet_outcome.winner == bet.player_a
                || bet_outcome.winner == bet.player_b,
            BetError::InvalidWinner
        );

        let message = serialize_outcome(&bet_outcome);

        let secp_ix = find_secp256k1(&ctx.accounts.instructions)?;

        let (ix_msg, ix_oracle) = parse_secp256k1(&secp_ix.data)?;

        require!(ix_msg == message, BetError::MessageMismatch);
        require!(ix_oracle == bet.oracle, BetError::OracleMismatch);

        bet.resolved = true;
        bet.winner = bet_outcome.winner;

        msg!(
            "Bet {} resolved. Winner: {:?}",
            bet.bet_id,
            bet_outcome.winner
        );

        Ok(())
    }

    pub fn claim_winnings(ctx: Context<ClaimWinnings>) -> Result<()> {
        let bet = &ctx.accounts.bet;
        require!(bet.resolved, BetError::NotResolved);
        require!(
            ctx.accounts.winner.key() == bet.winner,
            BetError::NotWinner
        );

        let total = bet.amount.checked_mul(2).unwrap();
        **bet.to_account_info().try_borrow_mut_lamports()? -= total;
        **ctx.accounts.winner.try_borrow_mut_lamports()? += total;

        msg!(
            "Claimed {} lamports for winner {:?}",
            total,
            bet.winner
        );

        Ok(())
    }
}

fn serialize_outcome(outcome: &BetOutcome) -> Vec<u8> {
    let mut data = Vec::with_capacity(40);
    data.extend_from_slice(&outcome.bet_id.to_le_bytes());
    data.extend_from_slice(&outcome.winner.to_bytes());
    data
}

fn transfer_from<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    let ix = solana_program::system_instruction::transfer(
        from.key,
        to.key,
        amount,
    );
    solana_program::program::invoke(&ix, &[from.clone(), to.clone()])?;
    Ok(())
}

fn find_secp256k1(acc: &AccountInfo) -> Result<Instruction> {
    let data = acc
        .try_borrow_data()
        .map_err(|_| BetError::IntrospectionError)?;

    for i in 0..16u16 {
        match load_instruction_at(i as usize, &**data) {
            Ok(ix) if ix.program_id == SECP256K1_PROGRAM_ID => return Ok(ix),
            Ok(_) => continue,
            Err(_) => break,
        }
    }
    err!(BetError::Secp256k1InstructionNotFound)
}

fn parse_secp256k1(data: &[u8]) -> Result<(Vec<u8>, [u8; 20])> {
    if data.len() < 12 {
        return err!(BetError::IntrospectionError);
    }

    // bincode layout: count(u8) | sig_offset(u16) | sig_ix(u8) | pk_offset(u16) | pk_ix(u8) | msg_offset(u16) | msg_size(u16) | msg_ix(u8)
    let _sig_off = u16::from_le_bytes([data[1], data[2]]) as usize;
    let pk_off = u16::from_le_bytes([data[4], data[5]]) as usize;
    let msg_off = u16::from_le_bytes([data[7], data[8]]) as usize;
    let msg_len = u16::from_le_bytes([data[9], data[10]]) as usize;

    let msg_end = msg_off.checked_add(msg_len).ok_or(BetError::IntrospectionError)?;
    let pk_end = pk_off.checked_add(20).ok_or(BetError::IntrospectionError)?;

    if data.len() < pk_end || data.len() < msg_end {
        return err!(BetError::IntrospectionError);
    }

    let msg = data[msg_off..msg_end].to_vec();
    let mut oracle = [0u8; 20];
    oracle.copy_from_slice(&data[pk_off..pk_end]);

    Ok((msg, oracle))
}
