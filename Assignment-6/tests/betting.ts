import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { Betting } from "../target/types/betting";
import { expect, use } from "chai";
import { keccak_256 } from "@noble/hashes/sha3";
import { secp256k1 } from "@noble/curves/secp256k1";
import {
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";

const SECP256K1_PROGRAM_ID = new PublicKey(
  "KeccakSecp256k11111111111111111111111111111"
);

describe("betting", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Betting as Program<Betting>;

  const AIRDROP = 5 * LAMPORTS_PER_SOL;
  const BET_AMOUNT = LAMPORTS_PER_SOL;

  function serializeBetOutcome(
    betId: BN,
    winner: PublicKey
  ): Buffer {
    const buf = Buffer.alloc(40);
    buf.writeBigUInt64LE(BigInt(betId.toString()), 0);
    buf.set(winner.toBytes(), 8);
    return buf;
  }

  function createSecp256k1Instruction(
    message: Buffer,
    privateKey: Uint8Array,
    ethAddress: Uint8Array
  ): TransactionInstruction {
    const msgHash = keccak_256(message);

    const sig = secp256k1.sign(msgHash, privateKey);

    const sigBytes = sig.toCompactRawBytes();
    const recoveryId = sig.recovery ?? 0;

    const DATA_START = 12;
    const pkOffset = DATA_START;
    const sigOffset = pkOffset + 20;
    const recoveryOffset = sigOffset + 64;
    const msgOffset = recoveryOffset + 1;

    const data = Buffer.alloc(msgOffset + message.length);

    let off = 0;
    data[off++] = 1; // num_signatures (u8)
    data.writeUInt16LE(sigOffset, off); off += 2; // sig_offset
    data[off++] = 0; // sig_instruction_index (u8)
    data.writeUInt16LE(pkOffset, off); off += 2; // pk_offset
    data[off++] = 0; // pk_instruction_index (u8)
    data.writeUInt16LE(msgOffset, off); off += 2; // msg_offset
    data.writeUInt16LE(message.length, off); off += 2; // msg_size
    data[off++] = 0; // msg_instruction_index (u8)

    data.set(ethAddress, pkOffset);
    data.set(sigBytes, sigOffset);
    data[recoveryOffset] = recoveryId;
    data.set(message, msgOffset);

    return new TransactionInstruction({
      programId: SECP256K1_PROGRAM_ID,
      keys: [],
      data,
    });
  }

  async function airdrop(pubkey: PublicKey): Promise<void> {
    const sig = await provider.connection.requestAirdrop(pubkey, AIRDROP);
    await provider.connection.confirmTransaction(sig, "confirmed");
  }

  it("CreateBet - initializes a bet between two players", async () => {
    const playerA = anchor.web3.Keypair.generate();
    const playerB = anchor.web3.Keypair.generate();
    await airdrop(playerA.publicKey);
    await airdrop(playerB.publicKey);

    const oraclePrivateKey = secp256k1.utils.randomPrivateKey();
    const oraclePubKey = secp256k1.getPublicKey(oraclePrivateKey, false);
    const oracleAddress = keccak_256(oraclePubKey.slice(1)).slice(12);

    const betId = new BN(1);

    const [betPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("bet"), new Uint8Array(betId.toArrayLike(Buffer, "le", 8))],
      program.programId
    );

    await program.methods
      .createBet(betId, new BN(BET_AMOUNT), Array.from(oracleAddress))
      .accounts({
        bet: betPda,
        playerA: playerA.publicKey,
        playerB: playerB.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerA])
      .rpc({ skipPreflight: true });

    const bet = await program.account.bet.fetch(betPda);
    expect(bet.betId.toString()).to.equal(betId.toString());
    expect(bet.playerA.toBase58()).to.equal(playerA.publicKey.toBase58());
    expect(bet.playerB.toBase58()).to.equal(playerB.publicKey.toBase58());
    expect(bet.amount.toNumber()).to.equal(BET_AMOUNT);
    expect(Array.from(bet.oracle)).to.deep.equal(Array.from(oracleAddress));
    expect(bet.resolved).to.be.false;
  });

  it("JoinBet - player B deposits their stake", async () => {
    const playerA = anchor.web3.Keypair.generate();
    const playerB = anchor.web3.Keypair.generate();
    await airdrop(playerA.publicKey);
    await airdrop(playerB.publicKey);

    const oraclePrivateKey = secp256k1.utils.randomPrivateKey();
    const oraclePubKey = secp256k1.getPublicKey(oraclePrivateKey, false);
    const oracleAddress = keccak_256(oraclePubKey.slice(1)).slice(12);

    const betId = new BN(2);

    const [betPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("bet"), new Uint8Array(betId.toArrayLike(Buffer, "le", 8))],
      program.programId
    );

    await program.methods
      .createBet(betId, new BN(BET_AMOUNT), Array.from(oracleAddress))
      .accounts({
        bet: betPda,
        playerA: playerA.publicKey,
        playerB: playerB.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerA])
      .rpc({ skipPreflight: true });

    const playerBBalanceBefore = await provider.connection.getBalance(
      playerB.publicKey
    );

    await program.methods
      .joinBet()
      .accounts({
        bet: betPda,
        playerB: playerB.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerB])
      .rpc({ skipPreflight: true });

    const bet = await program.account.bet.fetch(betPda);
    expect(bet.resolved).to.be.false;

    const betBal = await provider.connection.getBalance(betPda);
    expect(betBal).to.be.at.least(BET_AMOUNT * 2);
  });

  it("ResolveBet - resolves bet with secp256k1 instruction introspection", async () => {
    const playerA = anchor.web3.Keypair.generate();
    const playerB = anchor.web3.Keypair.generate();
    await airdrop(playerA.publicKey);
    await airdrop(playerB.publicKey);

    const oraclePrivateKey = secp256k1.utils.randomPrivateKey();
    const oraclePubKey = secp256k1.getPublicKey(oraclePrivateKey, false);
    const oracleAddress = keccak_256(oraclePubKey.slice(1)).slice(12);

    const betId = new BN(3);

    const [betPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("bet"), new Uint8Array(betId.toArrayLike(Buffer, "le", 8))],
      program.programId
    );

    await program.methods
      .createBet(betId, new BN(BET_AMOUNT), Array.from(oracleAddress))
      .accounts({
        bet: betPda,
        playerA: playerA.publicKey,
        playerB: playerB.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerA])
      .rpc({ skipPreflight: true });

    await program.methods
      .joinBet()
      .accounts({
        bet: betPda,
        playerB: playerB.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerB])
      .rpc({ skipPreflight: true });

    const winner = playerA.publicKey;
    const betOutcome = { betId, winner };
    const message = serializeBetOutcome(betId, winner);

    const secpIx = createSecp256k1Instruction(
      message,
      oraclePrivateKey,
      oracleAddress
    );

    const resolveIx = await program.methods
      .resolveBet(betOutcome)
      .accounts({
        bet: betPda,
        instructions: SYSVAR_INSTRUCTIONS_PUBKEY,
        caller: provider.wallet.publicKey,
      })
      .instruction();

    const tx = new Transaction();
    tx.add(secpIx);
    tx.add(resolveIx);

    const txSig = await anchor.web3.sendAndConfirmTransaction(
      provider.connection,
      tx,
      [provider.wallet.payer],
      { skipPreflight: true }
    );
    console.log("ResolveBet tx:", txSig);

    const bet = await program.account.bet.fetch(betPda);
    expect(bet.resolved).to.be.true;
    expect(bet.winner.toBase58()).to.equal(winner.toBase58());
  });

  it("ResolveBet - rejects wrong message in secp256k1 instruction", async () => {
    const playerA = anchor.web3.Keypair.generate();
    const playerB = anchor.web3.Keypair.generate();
    await airdrop(playerA.publicKey);
    await airdrop(playerB.publicKey);

    const oraclePrivateKey = secp256k1.utils.randomPrivateKey();
    const oraclePubKey = secp256k1.getPublicKey(oraclePrivateKey, false);
    const oracleAddress = keccak_256(oraclePubKey.slice(1)).slice(12);

    const betId = new BN(4);

    const [betPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("bet"), new Uint8Array(betId.toArrayLike(Buffer, "le", 8))],
      program.programId
    );

    await program.methods
      .createBet(betId, new BN(BET_AMOUNT), Array.from(oracleAddress))
      .accounts({
        bet: betPda,
        playerA: playerA.publicKey,
        playerB: playerB.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerA])
      .rpc({ skipPreflight: true });

    await program.methods
      .joinBet()
      .accounts({
        bet: betPda,
        playerB: playerB.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerB])
      .rpc({ skipPreflight: true });

    const fakeOutcome = { betId: new BN(999), winner: playerB.publicKey };
    const fakeMessage = serializeBetOutcome(fakeOutcome.betId, fakeOutcome.winner);

    const secpIx = createSecp256k1Instruction(
      fakeMessage,
      oraclePrivateKey,
      oracleAddress
    );

    const realOutcome = { betId, winner: playerA.publicKey };
    const resolveIx = await program.methods
      .resolveBet(realOutcome)
      .accounts({
        bet: betPda,
        instructions: SYSVAR_INSTRUCTIONS_PUBKEY,
        caller: provider.wallet.publicKey,
      })
      .instruction();

    const tx = new Transaction();
    tx.add(secpIx);
    tx.add(resolveIx);

    try {
      await anchor.web3.sendAndConfirmTransaction(
        provider.connection,
        tx,
        [provider.wallet.payer],
        { skipPreflight: true }
      );
      expect.fail("Should have thrown an error");
    } catch (_err) {
      const bet = await program.account.bet.fetch(betPda);
      expect(bet.resolved).to.be.false;
    }
  });

  it("ResolveBet - rejects wrong oracle address", async () => {
    const playerA = anchor.web3.Keypair.generate();
    const playerB = anchor.web3.Keypair.generate();
    await airdrop(playerA.publicKey);
    await airdrop(playerB.publicKey);

    const oraclePrivateKey = secp256k1.utils.randomPrivateKey();
    const oraclePubKey = secp256k1.getPublicKey(oraclePrivateKey, false);
    const oracleAddress = keccak_256(oraclePubKey.slice(1)).slice(12);

    const betId = new BN(5);

    const [betPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("bet"), new Uint8Array(betId.toArrayLike(Buffer, "le", 8))],
      program.programId
    );

    await program.methods
      .createBet(betId, new BN(BET_AMOUNT), Array.from(oracleAddress))
      .accounts({
        bet: betPda,
        playerA: playerA.publicKey,
        playerB: playerB.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerA])
      .rpc({ skipPreflight: true });

    await program.methods
      .joinBet()
      .accounts({
        bet: betPda,
        playerB: playerB.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerB])
      .rpc({ skipPreflight: true });

    const badOracle = secp256k1.utils.randomPrivateKey();
    const badOraclePubKey = secp256k1.getPublicKey(badOracle, false);
    const badOracleAddress = keccak_256(badOraclePubKey.slice(1)).slice(12);

    const betOutcome = { betId, winner: playerA.publicKey };
    const message = serializeBetOutcome(betId, playerA.publicKey);

    const secpIx = createSecp256k1Instruction(
      message,
      badOracle,
      badOracleAddress
    );

    const resolveIx = await program.methods
      .resolveBet(betOutcome)
      .accounts({
        bet: betPda,
        instructions: SYSVAR_INSTRUCTIONS_PUBKEY,
        caller: provider.wallet.publicKey,
      })
      .instruction();

    const tx = new Transaction();
    tx.add(secpIx);
    tx.add(resolveIx);

    try {
      await anchor.web3.sendAndConfirmTransaction(
        provider.connection,
        tx,
        [provider.wallet.payer],
        { skipPreflight: true }
      );
      expect.fail("Should have thrown an error");
    } catch (_err) {
      const bet = await program.account.bet.fetch(betPda);
      expect(bet.resolved).to.be.false;
    }
  });

  it("ClaimWinnings - winner receives total pot", async () => {
    const playerA = anchor.web3.Keypair.generate();
    const playerB = anchor.web3.Keypair.generate();
    await airdrop(playerA.publicKey);
    await airdrop(playerB.publicKey);

    const oraclePrivateKey = secp256k1.utils.randomPrivateKey();
    const oraclePubKey = secp256k1.getPublicKey(oraclePrivateKey, false);
    const oracleAddress = keccak_256(oraclePubKey.slice(1)).slice(12);

    const betId = new BN(6);

    const [betPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("bet"), new Uint8Array(betId.toArrayLike(Buffer, "le", 8))],
      program.programId
    );

    await program.methods
      .createBet(betId, new BN(BET_AMOUNT), Array.from(oracleAddress))
      .accounts({
        bet: betPda,
        playerA: playerA.publicKey,
        playerB: playerB.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerA])
      .rpc({ skipPreflight: true });

    await program.methods
      .joinBet()
      .accounts({
        bet: betPda,
        playerB: playerB.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerB])
      .rpc({ skipPreflight: true });

    const winner = playerA.publicKey;
    const betOutcome = { betId, winner };
    const message = serializeBetOutcome(betId, winner);

    const secpIx = createSecp256k1Instruction(
      message,
      oraclePrivateKey,
      oracleAddress
    );

    const resolveIx = await program.methods
      .resolveBet(betOutcome)
      .accounts({
        bet: betPda,
        instructions: SYSVAR_INSTRUCTIONS_PUBKEY,
        caller: provider.wallet.publicKey,
      })
      .instruction();

    const resolveTx = new Transaction();
    resolveTx.add(secpIx);
    resolveTx.add(resolveIx);

    await anchor.web3.sendAndConfirmTransaction(
      provider.connection,
      resolveTx,
      [provider.wallet.payer],
      { skipPreflight: true }
    );

    const balanceBefore = await provider.connection.getBalance(winner);

    await program.methods
      .claimWinnings()
      .accounts({
        bet: betPda,
        winner: winner,
        systemProgram: SystemProgram.programId,
      })
      .signers([playerA])
      .rpc({ skipPreflight: true });

    const balanceAfter = await provider.connection.getBalance(winner);
    const betBalAfter = await provider.connection.getBalance(betPda);

    expect(balanceAfter - balanceBefore).to.be.at.least(BET_AMOUNT * 2);
    expect(betBalAfter).to.equal(0);
  });
});
