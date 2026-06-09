# NFT Marketplace

A decentralized NFT marketplace built on Solana using the [Anchor](https://www.anchor-lang.com/) framework. Supports listing, buying, and offering on NFTs with both SOL and SPL tokens as payment.

## Features

- **Initialize** – Bootstrap the marketplace with an admin, treasury, fee basis points, and rewards mint
- **List / List with Token** – List an NFT for sale in SOL or a specific SPL token
- **Buy / Buy with Token** – Purchase a listed NFT. Payment is split: seller receives price minus fee, treasury collects the fee
- **Delist** – Cancel an active listing and return the NFT to the maker
- **Make Offer** – Escrow SOL into a PDA as a binding offer on any NFT
- **Accept Offer** – Accept an outstanding offer; escrowed SOL is distributed to the asset owner (minus fee) and treasury
- **Cancel Offer** – Withdraw an offer and reclaim escrowed SOL

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Blockchain | Solana |
| Framework | Anchor 0.31.0 |
| Smart Contract | Rust (anchor-lang + anchor-spl) |
| Tests | TypeScript, Mocha, Chai |
| Token Standard | SPL Token / SPL Associated Token Account |

## Prerequisites

- [Solana CLI](https://docs.solanalabs.com/cli/install) (v1.17+)
- [Anchor CLI](https://www.anchor-lang.com/docs/installation) (v0.31.0)
- Node.js 18+ and Yarn

## Project Structure

```
nft-marketplace/
├── programs/
│   └── nft_marketplace/
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs              # On-chain program logic
├── tests/
│   └── nft_marketplace.ts          # Integration tests
├── migrations/
│   └── deploy.ts                   # Deploy script
├── app/                             # Frontend (placeholder)
├── Anchor.toml                      # Anchor configuration
├── Cargo.toml                       # Rust workspace
├── package.json                     # JS dependencies
└── tsconfig.json
```

## Getting Started

1. Install dependencies:

```bash
yarn install
```

2. Build the program:

```bash
anchor build
```

3. Start a local validator:

```bash
solana-test-validator
```

4. Deploy the program:

```bash
anchor deploy
```

5. Run tests:

```bash
anchor test
```

## Program Instructions

| Instruction | Description |
|-------------|-------------|
| `initialize` | Create the marketplace config PDA |
| `list` | List an NFT for sale in SOL |
| `list_with_token` | List an NFT for sale in an SPL token |
| `buy` | Buy an NFT listed in SOL |
| `buy_with_token` | Buy an NFT listed in an SPL token |
| `delist` | Remove an active listing |
| `make_offer` | Place a SOL offer on an NFT |
| `accept_offer` | Accept an offer and transfer the NFT |
| `cancel_offer` | Cancel an offer and reclaim SOL |

## Accounts

### Config
PDA (`seeds = ["config"]`) storing admin, treasury, fee bps, rewards mint, and initialization flag.

### Listing
PDA (`seeds = ["listing", asset_mint, maker]`) storing maker, asset mint, price, payment mint (empty for SOL), and bump.

### Offer
PDA (`seeds = ["offer", asset_mint, buyer]`) storing buyer, asset mint, amount, and bump. SOL is escrowed into this account upon creation.

## Fee Structure

Fees are denominated in basis points (e.g., `200` = 2%). On each sale:
- Seller receives `price - fee`
- Treasury collects `fee`

## License

ISC
