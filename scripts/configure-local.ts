import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { AnchorProvider, Idl, Program, setProvider } from '@coral-xyz/anchor';
import bs58 from 'bs58';
import { Connection, Keypair, PublicKey, SystemProgram } from '@solana/web3.js';

function required(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`Missing environment variable ${name}`);
  return value;
}

function clusterDomain(genesisHash: string, label: string): Buffer {
  return createHash('sha256')
    .update('ROUTERPULSE_CLUSTER_V1')
    .update(bs58.decode(genesisHash))
    .update(':')
    .update(label)
    .digest();
}

const localName = required('ROUTERPULSE_CLUSTER_NAME');
const remoteName = required('ROUTERPULSE_REMOTE_CLUSTER_NAME');
const remoteRpcUrl = required('ROUTERPULSE_REMOTE_RPC_URL');
const relayerKeyPath = required('ROUTERPULSE_RELAYER_KEYPAIR');
const idl = JSON.parse(await readFile(resolve('target/idl/routerpulse.json'), 'utf8')) as Idl;
const provider = AnchorProvider.env();
setProvider(provider);
const program = new Program(idl, provider);
const operator = provider.wallet.publicKey;
const relayerSecret = JSON.parse(await readFile(relayerKeyPath, 'utf8')) as number[];
const relayer = Keypair.fromSecretKey(Uint8Array.from(relayerSecret));
const remoteConnection = new Connection(remoteRpcUrl, 'confirmed');

const localGenesis = await provider.connection.getGenesisHash();
const remoteGenesis = await remoteConnection.getGenesisHash();
const localDomain = clusterDomain(localGenesis, localName);
const remoteDomain = clusterDomain(remoteGenesis, remoteName);
if (localDomain.equals(remoteDomain)) throw new Error('Source and destination domain hashes must be distinct.');

const [config] = PublicKey.findProgramAddressSync([Buffer.from('config')], program.programId);
const [remote] = PublicKey.findProgramAddressSync(
  [Buffer.from('remote'), remoteDomain, program.programId.toBuffer()], program.programId,
);

if (!(await provider.connection.getAccountInfo(config))) {
  await program.methods.initialize([...localDomain], relayer.publicKey)
    .accounts({ payer: operator, authority: operator, config, systemProgram: SystemProgram.programId })
    .rpc();
} else {
  const configState = await program.account.routerConfig.fetch(config);
  if (!configState.relayer.equals(relayer.publicKey)) {
    await program.methods.setRelayer(relayer.publicKey).accounts({ authority: operator, config }).rpc();
  }
}

if (!(await provider.connection.getAccountInfo(remote))) {
  await program.methods.registerRemote([...remoteDomain], program.programId, true)
    .accounts({ authority: operator, config, remote, systemProgram: SystemProgram.programId })
    .rpc();
} else {
  await program.methods.updateRemote([...remoteDomain], program.programId, true)
    .accounts({ authority: operator, config, remote }).rpc();
}

console.log(`Configured ${localName} -> ${remoteName}`);
console.log(`Local domain:  ${localDomain.toString('hex')}`);
console.log(`Remote domain: ${remoteDomain.toString('hex')}`);
