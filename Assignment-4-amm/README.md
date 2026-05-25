# Solana AMM Program

This project is a simple constant-product AMM built with Anchor. It uses two SOL vaults and a small LP account per user to track shares.

## Features

- Initialize pool
- Add liquidity
- Remove liquidity
- Swap

## Run Locally

### Build Program

```bash
NO_DNA=1 anchor build
```

### Run Tests

```bash
NO_DNA=1 anchor test
```

## Test Cases

- Initialize pool
- Add liquidity
- Remove liquidity
- Swap

## Screenshot for Submission

After running `anchor test`, take a screenshot showing:

```
amm
  ok test_initialize_pool
  ok test_add_liquidity
  ok test_remove_liquidity
  ok test_swap_a_to_b

4 passing
```

Add the screenshot to the repo, for example: `assets/tests-passing.png`.

## Deployment

- Cluster: devnet
- Program ID: CJZU4fsC4LZGkCyRw3AoMKM8y1GQWGCmnuu5kTBbMece

## Important Fix

If you regenerate the program keypair, update `declare_id!` and the `Anchor.toml` program id to match.

## Author

Kartik Vyas
