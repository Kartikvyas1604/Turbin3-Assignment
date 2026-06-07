# Assignment 5 - NFT Staking (Anchor)

This repository contains a from-scratch Anchor program implementing NFT staking with:

- A dedicated `claim_rewards` instruction (independent from `unstake_nft`)
- A collection-level Attributes plugin account that tracks number of currently staked NFTs
- Full tests covering all instructions

## Features Implemented

- `initialize_config`
  - Creates staking config PDA
  - Stores reward emission rate (`reward_rate_per_second`)
  - Initializes collection attributes plugin account with:
    - `staked_nfts = 0`
- `stake_nft`
  - Transfers NFT from user ATA to a vault ATA controlled by PDA
  - Creates/updates stake record (`staked_at`, `last_claimed_at`, `active`)
  - Increments collection plugin attribute `staked_nfts`
- `claim_rewards`
  - Computes rewards from elapsed seconds since `last_claimed_at`
  - Mints SPL reward tokens to staker reward ATA
  - Updates `last_claimed_at`
  - Does **not** unstake NFT
- `unstake_nft`
  - Transfers NFT back from vault ATA to staker ATA
  - Marks stake inactive
  - Decrements collection plugin attribute `staked_nfts`

## Why this satisfies the challenge

- Users can claim rewards without unstaking (`claim_rewards` leaves `active = true` and NFT in vault)
- Users can unstake right after claiming rewards (unstake does not force reward re-claim logic)
- Collection-level stake count is persisted in the Attributes plugin account under key `staked_nfts`

## Project Structure

- `programs/nft_staking/src/lib.rs` - on-chain program
- `tests/nft_staking.ts` - integration test covering initialize, stake, claim, unstake
- `Anchor.toml` - Anchor workspace configuration

## Prerequisites

- Rust + Cargo
- Solana CLI
- Anchor CLI
- Node.js 18+

## Install

```bash
yarn install
```

## Run Tests

```bash
anchor test
```

## Test Coverage Notes

The integration test validates:

- Config/plugin initialization and initial `staked_nfts` value
- Stake flow and NFT custody transfer to vault
- Claim flow without unstake (reward minting + stake stays active)
- Unstake immediately after claim (reward amount unchanged, NFT returned)
- Plugin `staked_nfts` increments/decrements correctly

## Screenshot of Passing Tests

A screenshot is included at:

- `docs/tests-passing.png`

(Generated after running `anchor test` locally.)
