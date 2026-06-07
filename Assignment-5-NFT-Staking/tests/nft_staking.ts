import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { expect } from "chai";
import {
  TOKEN_PROGRAM_ID,
  createAssociatedTokenAccount,
  createMint,
  getAccount,
  getAssociatedTokenAddressSync,
  mintTo,
} from "@solana/spl-token";
import { NftStaking } from "../target/types/nft_staking";

describe("NFT Staking Program", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.NftStaking as Program<NftStaking>;
  const connection = provider.connection;
  const staker = provider.wallet as anchor.Wallet;

  const rewardRatePerSecond = new anchor.BN(10);

  let collectionMint: anchor.web3.PublicKey;
  let rewardMint: anchor.web3.PublicKey;
  let nftMint: anchor.web3.PublicKey;

  let stakerNftAta: anchor.web3.PublicKey;
  let stakerRewardAta: anchor.web3.PublicKey;
  let vaultNftAta: anchor.web3.PublicKey;

  const [configPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("config"), staker.publicKey.toBuffer()],
    program.programId
  );

  const [rewardMintAuthorityPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("reward-authority"), configPda.toBuffer()],
    program.programId
  );

  let collectionPluginPda: anchor.web3.PublicKey;
  let stakeRecordPda: anchor.web3.PublicKey;
  let vaultAuthorityPda: anchor.web3.PublicKey;

  const wait = async (ms: number) =>
    new Promise((resolve) => setTimeout(resolve, ms));

  before(async () => {
    // 1. Create the mints required
    collectionMint = await createMint(connection, staker.payer, staker.publicKey, null, 0);
    rewardMint = await createMint(connection, staker.payer, rewardMintAuthorityPda, null, 0);
    nftMint = await createMint(connection, staker.payer, staker.publicKey, null, 0);

    // 2. Setup staker's NFT ATA and mint 1 NFT
    stakerNftAta = await createAssociatedTokenAccount(
      connection,
      staker.payer,
      nftMint,
      staker.publicKey
    );
    await mintTo(connection, staker.payer, nftMint, stakerNftAta, staker.publicKey, 1);

    // 3. Derive remaining PDAs
    [collectionPluginPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("collection-plugin"), collectionMint.toBuffer()],
      program.programId
    );

    [stakeRecordPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("stake"), staker.publicKey.toBuffer(), nftMint.toBuffer()],
      program.programId
    );

    [vaultAuthorityPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), stakeRecordPda.toBuffer()],
      program.programId
    );

    vaultNftAta = getAssociatedTokenAddressSync(nftMint, vaultAuthorityPda, true);
    stakerRewardAta = getAssociatedTokenAddressSync(rewardMint, staker.publicKey);
  });

  describe("Initialization", () => {
    it("Initializes the staking configuration and attributes plugin", async () => {
      await program.methods
        .initializeConfig(rewardRatePerSecond)
        .accounts({
          authority: staker.publicKey,
          config: configPda,
          collectionMint,
          collectionPlugin: collectionPluginPda,
          rewardMint,
          rewardMintAuthority: rewardMintAuthorityPda,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();

      const configAccount = await program.account.config.fetch(configPda);
      expect(configAccount.rewardRatePerSecond.toNumber()).to.equal(rewardRatePerSecond.toNumber());

      const pluginAccount = await program.account.collectionAttributesPlugin.fetch(collectionPluginPda);
      expect(pluginAccount.attributes[0].key).to.equal("staked_nfts");
      expect(pluginAccount.attributes[0].value).to.equal("0");
    });
  });

  describe("Staking", () => {
    it("Successfully stakes a valid NFT", async () => {
      await program.methods
        .stakeNft()
        .accounts({
          staker: staker.publicKey,
          config: configPda,
          collectionPlugin: collectionPluginPda,
          nftMint,
          stakerNftAta,
          vaultNftAta,
          vaultAuthority: vaultAuthorityPda,
          stakeRecord: stakeRecordPda,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();

      const stakerAta = await getAccount(connection, stakerNftAta);
      const vaultAta = await getAccount(connection, vaultNftAta);
      expect(Number(stakerAta.amount)).to.equal(0);
      expect(Number(vaultAta.amount)).to.equal(1);

      const record = await program.account.stakeRecord.fetch(stakeRecordPda);
      expect(record.active).to.be.true;

      const pluginAccount = await program.account.collectionAttributesPlugin.fetch(collectionPluginPda);
      expect(pluginAccount.attributes[0].value).to.equal("1");
    });

    it("Fails when trying to stake an already staked NFT", async () => {
      try {
        await program.methods
          .stakeNft()
          .accounts({
            staker: staker.publicKey,
            config: configPda,
            collectionPlugin: collectionPluginPda,
            nftMint,
            stakerNftAta,
            vaultNftAta,
            vaultAuthority: vaultAuthorityPda,
            stakeRecord: stakeRecordPda,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
            systemProgram: anchor.web3.SystemProgram.programId,
          })
          .rpc();
        expect.fail("Expected stake implementation to fail but it succeeded");
      } catch (e: any) {
        expect(e.message).to.include("AlreadyStaked");
      }
    });
  });

  describe("Rewards", () => {
    it("Claims rewards accurately without unstaking the NFT", async () => {
      // Wait for at least 1 second to accumulate some rewards
      await wait(1200);

      await program.methods
        .claimRewards()
        .accounts({
          staker: staker.publicKey,
          config: configPda,
          nftMint,
          stakeRecord: stakeRecordPda,
          rewardMint,
          rewardMintAuthority: rewardMintAuthorityPda,
          stakerRewardAta,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();

      const rewardsAta = await getAccount(connection, stakerRewardAta);
      expect(Number(rewardsAta.amount)).to.be.greaterThan(0);

      const record = await program.account.stakeRecord.fetch(stakeRecordPda);
      expect(record.active).to.be.true; // Ensure it stays active
    });
  });

  describe("Unstaking", () => {
    it("Successfully unstakes the NFT", async () => {
      const rewardsAtaBefore = await getAccount(connection, stakerRewardAta);
      const preUnstakeRewardAmount = Number(rewardsAtaBefore.amount);

      await program.methods
        .unstakeNft()
        .accounts({
          staker: staker.publicKey,
          config: configPda,
          nftMint,
          stakeRecord: stakeRecordPda,
          collectionPlugin: collectionPluginPda,
          stakerNftAta,
          vaultNftAta,
          vaultAuthority: vaultAuthorityPda,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();

      const stakerAta = await getAccount(connection, stakerNftAta);
      const vaultAta = await getAccount(connection, vaultNftAta);
      expect(Number(stakerAta.amount)).to.equal(1);
      expect(Number(vaultAta.amount)).to.equal(0);

      const record = await program.account.stakeRecord.fetch(stakeRecordPda);
      expect(record.active).to.be.false;

      const pluginAccount = await program.account.collectionAttributesPlugin.fetch(collectionPluginPda);
      expect(pluginAccount.attributes[0].value).to.equal("0");

      const rewardsAtaAfter = await getAccount(connection, stakerRewardAta);
      expect(Number(rewardsAtaAfter.amount)).to.equal(preUnstakeRewardAmount); // No extra rewards auto-claimed
    });

    it("Fails when trying to claim rewards on an inactive stake", async () => {
      try {
        await program.methods
          .claimRewards()
          .accounts({
            staker: staker.publicKey,
            config: configPda,
            nftMint,
            stakeRecord: stakeRecordPda,
            rewardMint,
            rewardMintAuthority: rewardMintAuthorityPda,
            stakerRewardAta,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
            systemProgram: anchor.web3.SystemProgram.programId,
          })
          .rpc();
        expect.fail("Expected claim to fail since stake is inactive");
      } catch (e: any) {
        expect(e.message).to.include("StakeInactive");
      }
    });

    it("Fails when trying to unstake an inactive stake", async () => {
      try {
        await program.methods
          .unstakeNft()
          .accounts({
            staker: staker.publicKey,
            config: configPda,
            nftMint,
            stakeRecord: stakeRecordPda,
            collectionPlugin: collectionPluginPda,
            stakerNftAta,
            vaultNftAta,
            vaultAuthority: vaultAuthorityPda,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
            systemProgram: anchor.web3.SystemProgram.programId,
          })
          .rpc();
        expect.fail("Expected unstake to fail since stake is inactive");
      } catch (e: any) {
        expect(e.message).to.include("StakeInactive");
      }
    });
  });
});