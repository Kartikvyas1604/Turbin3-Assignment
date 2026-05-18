# Escrow Program (SOL)

A simple Anchor escrow program that locks SOL from a maker, lets a taker fulfill the deal, and supports maker refunds. Tests are written in Rust using LiteSVM.

## Features

- Make: maker locks SOL in escrow and sets a price.
- Take: taker pays the price and receives the escrowed SOL.
- Refund: maker cancels and reclaims the escrowed SOL.

## Prerequisites

- Rust toolchain
- Solana CLI
- Anchor CLI

## Build

```bash
NO_DNA=1 anchor build
```

## Test (Rust + LiteSVM)

```bash
NO_DNA=1 cargo test -p escrow
```

## Program ID

- `CVHSKiTKrivVkVy1Z8M2E1nBEKVddisehdPs5eW9qgKw`

## Notes

- The escrow PDA is derived from the maker pubkey.
- The escrow account holds both state and escrowed lamports.

## Screenshot

After tests pass, add a screenshot to the repo, for example:

- `assets/tests-passing.png`

Then include it in this README:

```md
![Tests Passing](assets/tests-passing.png)
```
