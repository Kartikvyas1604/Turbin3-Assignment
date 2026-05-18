import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Vault } from "../target/types/vault";

describe("vault", () => {
  const provider = anchor.AnchorProvider.env();

  anchor.setProvider(provider);

  const program = anchor.workspace.Vault as Program<Vault>;

  const user = provider.wallet;

  const [vaultPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), user.publicKey.toBuffer()],
    program.programId
  );

  it("Initialize Vault", async () => {
    await program.methods
      .initialize()
      .accounts({
        user: user.publicKey,
        vault: vaultPda,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    console.log("Vault Initialized");
  });

  it("Deposit SOL", async () => {
    await program.methods
      .deposit(new anchor.BN(10000000))
      .accounts({
        user: user.publicKey,
        vault: vaultPda,
        owner: user.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    console.log("Deposit Success");
  });

  it("Withdraw SOL", async () => {
    await program.methods
      .withdraw(new anchor.BN(5000000))
      .accounts({
        user: user.publicKey,
        vault: vaultPda,
        owner: user.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    console.log("Withdraw Success");
  });

  it("Close Vault", async () => {
    await program.methods
      .closeVault()
      .accounts({
        user: user.publicKey,
        vault: vaultPda,
      })
      .rpc();

    console.log("Vault Closed");
  });
});
