# Solana Escrow Program

This project is a simple Solana escrow program built using the Anchor framework.

## Features

- Make: maker locks SOL in escrow and sets a price
- Take: taker pays the price and receives the escrowed SOL
- Refund: maker cancels and reclaims the escrowed SOL

## Tech Stack

- Solana
- Anchor
- Rust
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

- Make
- Take
- Refund

## Screenshot for Submission

After running `anchor test`, take a screenshot showing:

```
escrow
	ok Make
	ok Take
	ok Refund

3 passing
```

Add the screenshot to the repo, for example: `assets/tests-passing.png`.

## Deployment

- Cluster: devnet
- Program ID: TBD (set after deploy)

## Important Fix

If you regenerate the program keypair, update `declare_id!` and the `Anchor.toml` program id to match.

## Author

Kartik Vyas
