import { createClient } from '@solana/kit';
import { solanaDevnetRpc } from '@solana/kit-plugin-rpc';
import { signerFromFile } from '@solana/kit-plugin-signer';
import { systemProgram } from '@solana-program/system';
import { tokenProgram } from '@solana-program/token';
import { generateKeyPairSigner } from '@solana/signers';

const rpcUrl = process.env.RPC_URL;
const homeDir = process.env.HOME;
const keypairPath =
  process.env.KEYPAIR_PATH ?? (homeDir ? `${homeDir}/.config/solana/id.json` : '');

if (!keypairPath) {
  throw new Error('KEYPAIR_PATH is required when HOME is not set.');
}

const decimals = Number.parseInt(process.env.TOKEN_DECIMALS ?? '6', 10);
const amount = BigInt(process.env.TOKEN_AMOUNT ?? '1000');

const client = await createClient()
  .use(signerFromFile(keypairPath))
  .use(rpcUrl ? solanaDevnetRpc({ rpcUrl }) : solanaDevnetRpc())
  .use(systemProgram())
  .use(tokenProgram());

const mintSigner = await generateKeyPairSigner();
const freezeAuthority = process.env.FREEZE_AUTHORITY === 'none' ? null : client.identity.address;

const createMintPlan = client.token.instructions.createMint({
  newMint: mintSigner,
  decimals,
  mintAuthority: client.identity.address,
  freezeAuthority,
});

const createMintSignature = await createMintPlan.sendTransaction();

const mintToPlan = client.token.instructions.mintToATA({
  mint: mintSigner.address,
  owner: client.identity.address,
  mintAuthority: client.identity,
  amount,
  decimals,
});

const mintToSignature = await mintToPlan.sendTransaction();

console.log('Mint address:', mintSigner.address);
console.log('Create mint signature:', createMintSignature);
console.log('Mint to ATA signature:', mintToSignature);
