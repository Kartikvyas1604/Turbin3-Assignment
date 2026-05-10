# Week 2 - SPL Token + MPL Core NFT

This repo contains two scripts:

- Mint an SPL token using Solana Kit.
- Mint an MPL Core NFT with the Attributes plugin.

## Prerequisites

- Node.js 18+
- Solana CLI installed and configured
- Devnet SOL in your wallet

## Setup

```bash
npm install
```

Copy the example env file and update values as needed.

```bash
cp .env.example .env
```

## Environment Variables

- `RPC_URL`: RPC endpoint (defaults to devnet).
- `KEYPAIR_PATH`: Path to your Solana CLI keypair JSON.
- `TOKEN_DECIMALS`: SPL token decimals (default: 6).
- `TOKEN_AMOUNT`: Amount to mint to your ATA (default: 1000).
- `FREEZE_AUTHORITY`: Set to `none` to make the mint non-freezable.
- `NFT_NAME`: Name for the MPL Core asset.
- `METADATA_URI`: Public URI for the NFT metadata JSON.

## Metadata

An example metadata file is provided at `assets/metadata.json`. Update the
`image` field to a public URL for `assets/image.png`, upload the JSON to
Arweave/IPFS, then set `METADATA_URI` to that JSON URL.

## Run

```bash
npm run mint:spl
npm run mint:nft
```

## Notes

- The NFT uses the MPL Core Attributes plugin.
- `METADATA_URI` should point to a valid JSON file that includes an `image` field and other standard metadata.
- Both scripts default to devnet. Override `RPC_URL` if needed.
