import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { NftMarketplace } from "../target/types/nft_marketplace";
import {
  createMint,
  mintTo,
  getOrCreateAssociatedTokenAccount,
  getAssociatedTokenAddress,
  getAccount,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";

describe("nft_marketplace", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.nftMarketplace as Program<NftMarketplace>;

  const { LAMPORTS_PER_SOL, SystemProgram, PublicKey } = anchor.web3;

  async function airdrop(
    pubkey: anchor.web3.PublicKey,
    amount: number = 10 * LAMPORTS_PER_SOL
  ) {
    const sig = await provider.connection.requestAirdrop(pubkey, amount);
    await provider.connection.confirmTransaction(sig);
  }

  async function getBalance(pubkey: anchor.web3.PublicKey): Promise<number> {
    return provider.connection.getBalance(pubkey);
  }

  async function getTokenBalance(
    ata: anchor.web3.PublicKey
  ): Promise<number> {
    const info = await getAccount(provider.connection, ata);
    return Number(info.amount);
  }

  const configPda = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    program.programId
  )[0];

  let admin: anchor.web3.Keypair;
  let treasury: anchor.web3.Keypair;
  let rewardsMint: anchor.web3.PublicKey;

  let maker: anchor.web3.Keypair;
  let buyer: anchor.web3.Keypair;
  let assetOwner: anchor.web3.Keypair;

  let nftMint: anchor.web3.PublicKey;
  let paymentMint: anchor.web3.PublicKey;
  const feeBps = 200; // 2%

  before(async () => {
    admin = anchor.web3.Keypair.generate();
    treasury = anchor.web3.Keypair.generate();
    maker = anchor.web3.Keypair.generate();
    buyer = anchor.web3.Keypair.generate();
    assetOwner = anchor.web3.Keypair.generate();

    await airdrop(admin.publicKey);
    await airdrop(treasury.publicKey);
    await airdrop(maker.publicKey);
    await airdrop(buyer.publicKey);
    await airdrop(assetOwner.publicKey);

    // Create rewards mint
    const rewardsMintKp = anchor.web3.Keypair.generate();
    rewardsMint = await createMint(
      provider.connection,
      admin,
      admin.publicKey,
      admin.publicKey,
      6,
      rewardsMintKp
    );

    // Create NFT mint (0 decimals for NFT)
    const nftMintKp = anchor.web3.Keypair.generate();
    nftMint = await createMint(
      provider.connection,
      admin,
      admin.publicKey,
      admin.publicKey,
      0,
      nftMintKp
    );

    // Create payment token mint (6 decimals like USDC)
    const paymentMintKp = anchor.web3.Keypair.generate();
    paymentMint = await createMint(
      provider.connection,
      admin,
      admin.publicKey,
      admin.publicKey,
      6,
      paymentMintKp
    );
  });

  it("1. Initialize marketplace", async () => {
    await program.methods
      .initialize(feeBps)
      .accounts({
        config: configPda,
        admin: admin.publicKey,
        treasury: treasury.publicKey,
        rewardsMint: rewardsMint,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin])
      .rpc();

    const config = await program.account.config.fetch(configPda);
    assert.equal(config.admin.toString(), admin.publicKey.toString());
    assert.equal(config.feeBps, feeBps);
    assert.equal(config.treasury.toString(), treasury.publicKey.toString());
    assert.equal(config.rewardsMint.toString(), rewardsMint.toString());
    assert.isTrue(config.isInitialized);
  });

  it("2. List and Buy with SOL", async () => {
    // Create maker's NFT ATA and mint 1 NFT
    const { address: makerNftAta } = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      maker,
      nftMint,
      maker.publicKey
    );

    await mintTo(
      provider.connection,
      maker,
      nftMint,
      makerNftAta,
      admin,
      1
    );

    const listingPda = PublicKey.findProgramAddressSync(
      [Buffer.from("listing"), nftMint.toBuffer(), maker.publicKey.toBuffer()],
      program.programId
    )[0];

    const vaultPda = await getAssociatedTokenAddress(
      nftMint,
      listingPda,
      true
    );

    const price = new BN(LAMPORTS_PER_SOL); // 1 SOL

    // List the NFT
    await program.methods
      .list(price)
      .accounts({
        listing: listingPda,
        maker: maker.publicKey,
        assetMint: nftMint,
        makerAta: makerNftAta,
        vault: vaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([maker])
      .rpc();

    // Verify listing
    const listing = await program.account.listing.fetch(listingPda);
    assert.equal(listing.maker.toString(), maker.publicKey.toString());
    assert.equal(listing.assetMint.toString(), nftMint.toString());
    assert.isTrue(listing.price.eq(price));

    // Verify NFT moved to vault
    const vaultBalance = await getTokenBalance(vaultPda);
    assert.equal(vaultBalance, 1);

    // Create buyer's NFT ATA
    const { address: buyerNftAta } = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      buyer,
      nftMint,
      buyer.publicKey
    );

    const makerBalanceBefore = await getBalance(maker.publicKey);
    const buyerBalanceBefore = await getBalance(buyer.publicKey);
    const treasuryBalanceBefore = await getBalance(treasury.publicKey);

    // Buy the NFT
    await program.methods
      .buy()
      .accounts({
        listing: listingPda,
        buyer: buyer.publicKey,
        maker: maker.publicKey,
        assetMint: nftMint,
        buyerAta: buyerNftAta,
        vault: vaultPda,
        config: configPda,
        treasury: treasury.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    // Verify NFT moved to buyer
    const buyerNftBalance = await getTokenBalance(buyerNftAta);
    assert.equal(buyerNftBalance, 1);

    // Verify vault is empty
    const vaultAfter = await getTokenBalance(vaultPda);
    assert.equal(vaultAfter, 0);

    // Verify SOL transfers
    const makerBalanceAfter = await getBalance(maker.publicKey);
    const buyerBalanceAfter = await getBalance(buyer.publicKey);
    const treasuryBalanceAfter = await getBalance(treasury.publicKey);

    const feeAmount = Math.floor(LAMPORTS_PER_SOL * feeBps / 10000);
    const makerAmount = LAMPORTS_PER_SOL - feeAmount;

    assert.isAbove(makerBalanceAfter, makerBalanceBefore);
    assert.isBelow(buyerBalanceAfter, buyerBalanceBefore);
    assert.equal(treasuryBalanceAfter - treasuryBalanceBefore, feeAmount);
  });

  it("3. List and Delist", async () => {
    // Create a new NFT mint for delist test
    const nftMintKp2 = anchor.web3.Keypair.generate();
    const nftMint2 = await createMint(
      provider.connection,
      admin,
      admin.publicKey,
      admin.publicKey,
      0,
      nftMintKp2
    );

    const { address: makerNftAta2 } = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      maker,
      nftMint2,
      maker.publicKey
    );

    await mintTo(
      provider.connection,
      maker,
      nftMint2,
      makerNftAta2,
      admin,
      1
    );

    const listingPda2 = PublicKey.findProgramAddressSync(
      [Buffer.from("listing"), nftMint2.toBuffer(), maker.publicKey.toBuffer()],
      program.programId
    )[0];

    const vaultPda2 = await getAssociatedTokenAddress(
      nftMint2,
      listingPda2,
      true
    );

    // List
    await program.methods
      .list(new BN(LAMPORTS_PER_SOL))
      .accounts({
        listing: listingPda2,
        maker: maker.publicKey,
        assetMint: nftMint2,
        makerAta: makerNftAta2,
        vault: vaultPda2,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([maker])
      .rpc();

    // Verify vault has NFT
    assert.equal(await getTokenBalance(vaultPda2), 1);

    // Delist
    await program.methods
      .delist()
      .accounts({
        listing: listingPda2,
        maker: maker.publicKey,
        assetMint: nftMint2,
        makerAta: makerNftAta2,
        vault: vaultPda2,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([maker])
      .rpc();

    // Verify NFT returned to maker
    assert.equal(await getTokenBalance(makerNftAta2), 1);
    assert.equal(await getTokenBalance(vaultPda2), 0);

    // Verify listing account is closed
    try {
      await program.account.listing.fetch(listingPda2);
      assert.fail("Listing should be closed");
    } catch (e) {
      // Expected error
    }
  });

  it("4. List and Buy with SPL tokens", async () => {
    // Create a new NFT mint for token buy test
    const nftMintKp3 = anchor.web3.Keypair.generate();
    const nftMint3 = await createMint(
      provider.connection,
      admin,
      admin.publicKey,
      admin.publicKey,
      0,
      nftMintKp3
    );

    const { address: makerNftAta3 } = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      maker,
      nftMint3,
      maker.publicKey
    );

    await mintTo(
      provider.connection,
      maker,
      nftMint3,
      makerNftAta3,
      admin,
      1
    );

    // Create buyer's payment token ATA and mint tokens
    const { address: buyerPaymentAta } = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      buyer,
      paymentMint,
      buyer.publicKey
    );

    await mintTo(
      provider.connection,
      admin,
      paymentMint,
      buyerPaymentAta,
      admin,
      1_000_000_000 // 1000 tokens (6 decimals)
    );

    const listingPda3 = PublicKey.findProgramAddressSync(
      [Buffer.from("listing"), nftMint3.toBuffer(), maker.publicKey.toBuffer()],
      program.programId
    )[0];

    const vaultPda3 = await getAssociatedTokenAddress(
      nftMint3,
      listingPda3,
      true
    );

    const tokenPrice = new BN(100_000_000); // 100 tokens

    // List with token
    await program.methods
      .listWithToken(tokenPrice)
      .accounts({
        listing: listingPda3,
        maker: maker.publicKey,
        assetMint: nftMint3,
        paymentMint: paymentMint,
        makerAta: makerNftAta3,
        vault: vaultPda3,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([maker])
      .rpc();

    // Verify listing
    const listing = await program.account.listing.fetch(listingPda3);
    assert.equal(listing.paymentMint.toString(), paymentMint.toString());
    assert.isTrue(listing.price.eq(tokenPrice));

    // Verify NFT in vault
    assert.equal(await getTokenBalance(vaultPda3), 1);

    // Create buyer's NFT ATA
    const { address: buyerNftAta3 } = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      buyer,
      nftMint3,
      buyer.publicKey
    );

    // Compute maker's payment ATA (will be created via init_if_needed)
    const makerPaymentAta = await getAssociatedTokenAddress(
      paymentMint,
      maker.publicKey
    );

    // Compute treasury's payment ATA (will be created via init_if_needed)
    const treasuryPaymentAta = await getAssociatedTokenAddress(
      paymentMint,
      treasury.publicKey
    );

    const buyerPaymentBefore = await getTokenBalance(buyerPaymentAta);

    // Buy with token
    await program.methods
      .buyWithToken()
      .accounts({
        listing: listingPda3,
        buyer: buyer.publicKey,
        maker: maker.publicKey,
        assetMint: nftMint3,
        paymentMint: paymentMint,
        buyerAta: buyerNftAta3,
        buyerPaymentAta: buyerPaymentAta,
        makerPaymentAta: makerPaymentAta,
        treasuryPaymentAta: treasuryPaymentAta,
        vault: vaultPda3,
        config: configPda,
        treasury: treasury.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    // Verify NFT transferred to buyer
    assert.equal(await getTokenBalance(buyerNftAta3), 1);

    // Verify payment token transfers
    const feeAmount = Math.floor(100_000_000 * feeBps / 10000);
    const makerAmount = 100_000_000 - feeAmount;

    const makerPaymentAfter = await getTokenBalance(makerPaymentAta);
    const treasuryPaymentAfter = await getTokenBalance(treasuryPaymentAta);
    assert.equal(makerPaymentAfter, makerAmount);
    assert.equal(treasuryPaymentAfter, feeAmount);

    const buyerPaymentAfter = await getTokenBalance(buyerPaymentAta);
    assert.equal(buyerPaymentBefore - buyerPaymentAfter, 100_000_000);
  });

  it("5. Make offer and Accept offer", async () => {
    // Create a new NFT mint for offer test
    const nftMintKp4 = anchor.web3.Keypair.generate();
    const nftMint4 = await createMint(
      provider.connection,
      admin,
      admin.publicKey,
      admin.publicKey,
      0,
      nftMintKp4
    );

    // Mint NFT to assetOwner
    const { address: assetOwnerNftAta } = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      assetOwner,
      nftMint4,
      assetOwner.publicKey
    );

    await mintTo(
      provider.connection,
      admin,
      nftMint4,
      assetOwnerNftAta,
      admin,
      1
    );

    const offerPda = PublicKey.findProgramAddressSync(
      [Buffer.from("offer"), nftMint4.toBuffer(), buyer.publicKey.toBuffer()],
      program.programId
    )[0];

    const offerAmount = new BN(2 * LAMPORTS_PER_SOL); // 2 SOL offer

    const buyerBalanceBefore = await getBalance(buyer.publicKey);

    // Make offer
    await program.methods
      .makeOffer(offerAmount)
      .accounts({
        offer: offerPda,
        buyer: buyer.publicKey,
        assetMint: nftMint4,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    // Verify offer
    const offer = await program.account.offer.fetch(offerPda);
    assert.equal(offer.buyer.toString(), buyer.publicKey.toString());
    assert.equal(offer.assetMint.toString(), nftMint4.toString());
    assert.isTrue(offer.amount.eq(offerAmount));

    // Verify SOL escrowed
    const offerBalance = await getBalance(offerPda);
    assert.isAtLeast(offerBalance, 2 * LAMPORTS_PER_SOL);

    const buyerBalanceMid = await getBalance(buyer.publicKey);
    assert.isBelow(buyerBalanceMid, buyerBalanceBefore);

    // Create buyer's NFT ATA for receiving the NFT
    const { address: buyerNftAta4 } = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      buyer,
      nftMint4,
      buyer.publicKey
    );

    const assetOwnerBalanceBefore = await getBalance(assetOwner.publicKey);
    const treasuryBalanceBefore = await getBalance(treasury.publicKey);

    // Accept offer (assetOwner signs, buyer does not)
    await program.methods
      .acceptOffer()
      .accounts({
        offer: offerPda,
        buyer: buyer.publicKey,
        assetOwner: assetOwner.publicKey,
        assetMint: nftMint4,
        assetOwnerAta: assetOwnerNftAta,
        buyerAta: buyerNftAta4,
        config: configPda,
        treasury: treasury.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([assetOwner])
      .rpc();

    // Verify NFT transferred to buyer
    assert.equal(await getTokenBalance(buyerNftAta4), 1);
    assert.equal(await getTokenBalance(assetOwnerNftAta), 0);

    // Verify SOL transfers
    const feeAmount = Math.floor(2 * LAMPORTS_PER_SOL * feeBps / 10000);
    const makerAmount = 2 * LAMPORTS_PER_SOL - feeAmount;

    const assetOwnerBalanceAfter = await getBalance(assetOwner.publicKey);
    const treasuryBalanceAfter = await getBalance(treasury.publicKey);

    assert.isAbove(
      assetOwnerBalanceAfter,
      assetOwnerBalanceBefore - LAMPORTS_PER_SOL
    );
    assert.equal(
      treasuryBalanceAfter - treasuryBalanceBefore,
      feeAmount
    );

    // Verify offer account closed
    try {
      await program.account.offer.fetch(offerPda);
      assert.fail("Offer should be closed");
    } catch (e) {
      // Expected
    }
  });

  it("6. Make offer and Cancel offer", async () => {
    // Create a new NFT mint for cancel test
    const nftMintKp5 = anchor.web3.Keypair.generate();
    const nftMint5 = await createMint(
      provider.connection,
      admin,
      admin.publicKey,
      admin.publicKey,
      0,
      nftMintKp5
    );

    const offerPda5 = PublicKey.findProgramAddressSync(
      [Buffer.from("offer"), nftMint5.toBuffer(), buyer.publicKey.toBuffer()],
      program.programId
    )[0];

    const offerAmount = new BN(LAMPORTS_PER_SOL); // 1 SOL

    await program.methods
      .makeOffer(offerAmount)
      .accounts({
        offer: offerPda5,
        buyer: buyer.publicKey,
        assetMint: nftMint5,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    // Verify offer exists
    const offer = await program.account.offer.fetch(offerPda5);
    assert.isTrue(offer.amount.eq(offerAmount));

    const buyerBalanceBeforeCancel = await getBalance(buyer.publicKey);

    // Cancel offer (close = buyer transfers all lamports back to buyer)
    await program.methods
      .cancelOffer()
      .accounts({
        offer: offerPda5,
        buyer: buyer.publicKey,
        assetMint: nftMint5,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    // Verify offer account closed
    try {
      await program.account.offer.fetch(offerPda5);
      assert.fail("Offer should be closed");
    } catch (e) {
      // Expected
    }

    // Verify SOL returned (close transfers all lamports including escrow + rent)
    const buyerBalanceAfterCancel = await getBalance(buyer.publicKey);
    assert.isAbove(buyerBalanceAfterCancel, buyerBalanceBeforeCancel);
  });
});
