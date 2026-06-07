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

describe("nft_staking", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.NftStaking as Program<NftStaking>;

  const staker = provider.wallet as anchor.Wallet;
  const connection = provider.connection;
  const rewardRatePerSecond = new anchor.BN(10);

  let collectionMint: anchor.web3.PublicKey;
  let rewardMint: anchor.web3.PublicKey;
  let nftMint: anchor.web3.PublicKey;

  let stakerNftAta: anchor.web3.PublicKey;
  let stakerRewardAta: anchor.web3.PublicKey;
  let vaultNftAta: anchor.web3.PublicKey;

  const [configPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("config"), staker.publicKey.toBuffer()],
    program.programId,
  );

  const [rewardMintAuthorityPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("reward-authority"), configPda.toBuffer()],
    program.programId,
  );

  let collectionPluginPda: anchor.web3.PublicKey;
  let stakeRecordPda: anchor.web3.PublicKey;
  let vaultAuthorityPda: anchor.web3.PublicKey;

  const wait = async (ms: number) =>
    new Promise((resolve) => setTimeout(resolve, ms));

  it("initializes config + attributes plugin, stakes NFT, claims rewards, and unstakes", async () => {
    collectionMint = await createMint(
      connection,
      staker.payer,
      staker.publicKey,
      null,
      0,
    );

    rewardMint = await createMint(
      connection,
      staker.payer,
      rewardMintAuthorityPda,
      null,
      0,
    );

    nftMint = await createMint(connection, staker.payer, staker.publicKey, null, 0);

    stakerNftAta = await createAssociatedTokenAccount(
      connection,
      staker.payer,
      nftMint,
      staker.publicKey,
    );

    await mintTo(
      connection,
      staker.payer,
      nftMint,
      stakerNftAta,
      staker.publicKey,
      1,
    );

    [collectionPluginPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("collection-plugin"), collectionMint.toBuffer()],
      program.programId,
    );

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

    const pluginAfterInit = await program.account.collectionAttributesPlugin.fetch(
      collectionPluginPda,
    );
    expect(pluginAfterInit.attributes[0].key).to.equal("staked_nfts");
    expect(pluginAfterInit.attributes[0].value).to.equal("0");

    [stakeRecordPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("stake"), staker.publicKey.toBuffer(), nftMint.toBuffer()],
      program.programId,
    );

    [vaultAuthorityPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), stakeRecordPda.toBuffer()],
      program.programId,
    );

    vaultNftAta = getAssociatedTokenAddressSync(nftMint, vaultAuthorityPda, true);

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

    const stakerNftAfterStake = await getAccount(connection, stakerNftAta);
    const vaultNftAfterStake = await getAccount(connection, vaultNftAta);
    expect(Number(stakerNftAfterStake.amount)).to.equal(0);
    expect(Number(vaultNftAfterStake.amount)).to.equal(1);

    const pluginAfterStake = await program.account.collectionAttributesPlugin.fetch(
      collectionPluginPda,
    );
    expect(pluginAfterStake.attributes[0].value).to.equal("1");

    await wait(1200);

    stakerRewardAta = getAssociatedTokenAddressSync(rewardMint, staker.publicKey);

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

    const rewardsAfterClaim = await getAccount(connection, stakerRewardAta);
    expect(Number(rewardsAfterClaim.amount)).to.be.greaterThan(0);

    const stakeRecordAfterClaim = await program.account.stakeRecord.fetch(stakeRecordPda);
    expect(stakeRecordAfterClaim.active).to.equal(true);

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

    const rewardsAfterUnstake = await getAccount(connection, stakerRewardAta);
    expect(Number(rewardsAfterUnstake.amount)).to.equal(Number(rewardsAfterClaim.amount));

    const stakerNftAfterUnstake = await getAccount(connection, stakerNftAta);
    const vaultNftAfterUnstake = await getAccount(connection, vaultNftAta);
    expect(Number(stakerNftAfterUnstake.amount)).to.equal(1);
    expect(Number(vaultNftAfterUnstake.amount)).to.equal(0);

    const pluginAfterUnstake = await program.account.collectionAttributesPlugin.fetch(
      collectionPluginPda,
    );
    expect(pluginAfterUnstake.attributes[0].value).to.equal("0");

    const finalStakeRecord = await program.account.stakeRecord.fetch(stakeRecordPda);
    expect(finalStakeRecord.active).to.equal(false);
  });
});
