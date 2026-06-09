# Betting Contract with Instruction Introspection

A Solana program demonstrating **instruction introspection** — validating that a secp256k1 signature verification instruction was included in the same transaction, rather than performing the signature check within the program itself.

## Challenge

> Write a contract that uses the instruction introspection concept, but use a different structure in the `ResolveBet` instruction, not the Ed25519 signature. Example: custom struct created by you.

## Architecture

The program implements a two-player betting contract:

1. **CreateBet** — Player A creates a bet, names Player B, and deposits SOL.
2. **JoinBet** — Player B deposits matching SOL to fund the pot.
3. **ResolveBet** — An off-chain oracle signs the outcome (as a `BetOutcome` struct) using a **secp256k1** keypair. The oracle's secp256k1 signature verification instruction is included in the same transaction. The program **introspects** the transaction's instruction list to verify that:
   - A secp256k1 verification instruction exists
   - The message in that instruction matches the serialized `BetOutcome`
   - The Ethereum-style address in that instruction matches the bet's configured oracle
4. **ClaimWinnings** — The winner withdraws the full pot (2× the bet amount).

## Key Concept: Instruction Introspection

Instead of verifying the signature inside the program itself, `ResolveBet` relies on Solana's **secp256k1 precompiled program** (`KeccakSecp256k11111111111111111111111111111`). The caller must include a secp256k1 instruction *before* the `resolveBet` instruction in the same transaction. If the secp256k1 verification fails, the entire transaction fails. The program then introspects via the Instructions sysvar to confirm the message and oracle address match the expected values.

## Custom Struct: `BetOutcome`

```rust
pub struct BetOutcome {
    pub bet_id: u64,
    pub winner: Pubkey,
}
```

The `BetOutcome` struct is serialized (40 bytes) and used as the signed message. This structured format replaces a raw Ed25519 signature with a richer, application-specific payload.

## Prerequisites

- Rust 1.75+
- Solana CLI 1.18+
- Anchor CLI 0.31+
- Node.js 18+

## Build & Test

```bash
# Install JS dependencies
yarn install

# Build the program
anchor build

# Run tests (starts local validator automatically)
anchor test
```

## Test Coverage

All tests use `skipPreflight: true` to avoid local simulation issues with the secp256k1 precompile.

| Test | Description |
|------|-------------|
| `CreateBet` | Creates a bet, verifies on-chain state |
| `JoinBet` | Player B deposits, verifies pot = 2× amount |
| `ResolveBet` | Resolves via secp256k1 introspection |
| `Rejects wrong message` | Ensures message mismatch is rejected |
| `Rejects wrong oracle` | Ensures wrong oracle address is rejected |
| `ClaimWinnings` | Winner claims full pot |
