import { readFile } from 'node:fs/promises';
import { createUmi } from '@metaplex-foundation/umi-bundle-defaults';
import { create, mplCore } from '@metaplex-foundation/mpl-core';
import {
  createSignerFromKeypair,
  generateSigner,
  keypairIdentity,
} from '@metaplex-foundation/umi';

const rpcUrl = process.env.RPC_URL ?? 'https://api.devnet.solana.com';
const homeDir = process.env.HOME;
const keypairPath =
  process.env.KEYPAIR_PATH ?? (homeDir ? `${homeDir}/.config/solana/id.json` : '');

if (!keypairPath) {
  throw new Error('KEYPAIR_PATH is required when HOME is not set.');
}

const name = process.env.NFT_NAME ?? 'Week 2 Core NFT';
const uri = process.env.METADATA_URI ?? 'https://example.com/asset.json';

const secretKey = Uint8Array.from(JSON.parse(await readFile(keypairPath, 'utf8')));

const umi = createUmi(rpcUrl).use(mplCore());
const keypair = umi.eddsa.createKeypairFromSecretKey(secretKey);
const signer = createSignerFromKeypair(umi, keypair);
umi.use(keypairIdentity(signer));

const assetSigner = generateSigner(umi);

const result = await create(umi, {
  name,
  uri,
  asset: assetSigner,
  plugins: [
    {
      type: 'Attributes',
      attributeList: [
        { key: 'course', value: 'week-2' },
        { key: 'tool', value: 'mpl-core' },
      ],
    },
  ],
}).sendAndConfirm(umi);

const signature = typeof result === 'string' ? result : result.signature;

console.log('Asset address:', assetSigner.publicKey);
console.log('Signature:', signature);
