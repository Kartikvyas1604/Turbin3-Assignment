# Solana Vault Program

This project is a simple Solana vault program built using Anchor framework.

## Features

- Initialize vault
- Deposit SOL
- Withdraw SOL
- Close vault

## Tech Stack

- Solana
- Anchor
- Typescript

## Run Locally

### Install dependencies

```bash
npm install
```

### Build Program

```bash
anchor build
```

### Run Tests

```bash
anchor test
```

## Test Cases

- Initialize vault
- Deposit SOL
- Withdraw SOL
- Close vault

## Screenshot for Submission

After running `anchor test`, take a screenshot showing:

```
vault
  ✔ Initialize Vault
  ✔ Deposit SOL
  ✔ Withdraw SOL
  ✔ Close Vault

4 passing
```

Add the screenshot to the repo, for example: `assets/tests-passing.png`.

## Deployment

- Cluster: devnet
- Program ID: BJZU4fsC4LZGkCyRw3AoMKM8y1GQWGCmnuu5kTBbMece

## Important Fix

If you regenerate the program keypair, update `declare_id!` and the `Anchor.toml` program id to match.

## Author

Kartik Vyas
