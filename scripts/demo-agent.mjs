#!/usr/bin/env node
/**
 * Bound — live testnet demo agent
 *
 * Goal: call guardian.spend with an account that should ONLY have a
 * Function Call access key scoped to method `spend`.
 *
 * Scenarios:
 *   1) Allowed — whitelisted vendor, amount <= max_per_tx & daily
 *   2) Rejected — amount > max_per_tx
 *   3) Rejected — receiver not whitelisted
 *
  * Phases: demo (default), --paused-check (after owner pauses), --negative-only (FC-key limit).
 *
 * Fill env vars (see README / .env.example). Do not commit private keys.
 */

import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));

function loadDotEnv() {
  for (const file of ['.env', '.env.local']) {
    const p = resolve(__dirname, file);
    if (!existsSync(p)) continue;
    for (const line of readFileSync(p, 'utf8').split('\n')) {
      const t = line.trim();
      if (!t || t.startsWith('#')) continue;
      const i = t.indexOf('=');
      if (i < 0) continue;
      const k = t.slice(0, i).trim();
      const v = t.slice(i + 1).trim();
      if (!(k in process.env)) process.env[k] = v;
    }
  }
}
loadDotEnv();

const CONFIG = {
  network: process.env.NEAR_NETWORK || 'testnet',
  rpc: process.env.NEAR_RPC_URL || 'https://rpc.testnet.near.org',
  contract: required('GUARDIAN_CONTRACT'),
  agentId: required('AGENT_ACCOUNT_ID'),
  vendor: required('VENDOR_ACCOUNT_ID'),
  scam: process.env.SCAM_ACCOUNT_ID || 'near-examples.testnet',
  amountOk: process.env.AMOUNT_OK || '100000000000000000000000',
  amountOverMax: process.env.AMOUNT_OVER_MAX || '300000000000000000000000',
};
function required(name) {
  const value = process.env[name];
  if (!value) throw new Error(`Missing ${name}; use .env.example and keep private keys in .env.local.`);
  return value;
}
async function main() {
  const { Account, JsonRpcProvider, KeyPairSigner } = await import('near-api-js');
  let privateKey = process.env.AGENT_PRIVATE_KEY;
  if (!privateKey) {
    const keyPath = process.env.AGENT_KEY_FILE || resolve(__dirname, 'credentials/agent-fc-key.json');
    const k = JSON.parse(readFileSync(keyPath, 'utf8'));
    privateKey = k.private_key || k.secret_key;
  }
  const signer = KeyPairSigner.fromSecretKey(privateKey);
  const account = new Account(CONFIG.agentId, CONFIG.rpc, signer);
  const provider = new JsonRpcProvider({ url: CONFIG.rpc });
  const view = (method, args = {}) => provider.callFunctionRaw({ contractId: CONFIG.contract, method, args })
    .then(({ result }) => JSON.parse(Buffer.from(result).toString()));
  const txHash = result => result.transaction?.hash || result.transaction_outcome?.id;
  const isTransferBlocked = error => error.name === 'AccessKeyRequiresFullAccessActionError' || error.type === 'RequiresFullAccess';
  console.log('Bound |', CONFIG.network, '|', CONFIG.agentId);
  console.log('Public policy:', await view('get_policy'));
  if (process.argv.includes('--paused-check')) {
    const result = await account.callFunctionRaw({ contractId: CONFIG.contract, methodName: 'spend', args: { receiver: CONFIG.vendor, amount: CONFIG.amountOk, memo: 'paused-check' }, gas: 30000000000000n, deposit: 0n });
    const log = (result.receipts_outcome || []).flatMap(x => x.outcome.logs || []).find(x => x.includes('SpendRejected'));
    if (!log || !log.includes('"reason":"paused"')) throw new Error(`Expected paused rejection; owner must pause first. ${log || ''}`);
    console.log(`PASS: paused request rejected, no transfer: https://explorer.testnet.near.org/transactions/${txHash(result)}`);
    return;
  }
  if (process.argv.includes('--negative-only')) {
    await account.transfer({ receiverId: CONFIG.vendor, amount: 1n }).then(() => { throw new Error('SECURITY FAILURE: direct transfer completed'); }).catch(error => {
      if (isTransferBlocked(error)) {
        console.log('PASS: native NEAR transfer blocked by Function Call key:', error.name || error.type);
        return;
      }
      throw error;
    });
    return;
  }
  const initialSpent = BigInt(await view('get_spent_today'));
  const daily = BigInt((await view('get_policy')).daily_limit);
  if (initialSpent + BigInt(CONFIG.amountOk) > daily) throw new Error('Insufficient daily headroom. Rerun after the UTC reset or configure a higher budget.');

  for (const [name, receiver, amount, expected] of [
    ['allowed', CONFIG.vendor, CONFIG.amountOk, 'SpendAllowed'],
    ['over-max', CONFIG.vendor, CONFIG.amountOverMax, 'SpendRejected'],
    ['not-whitelisted', CONFIG.scam, CONFIG.amountOk, 'SpendRejected'],
  ]) {
    const result = await account.callFunctionRaw({ contractId: CONFIG.contract, methodName: 'spend', args: { receiver, amount, memo: `bound-${name}` }, gas: 30000000000000n, deposit: 0n });
    const log = (result.receipts_outcome || []).flatMap(x => x.outcome.logs || []).find(x => x.startsWith('EVENT_JSON:'));
    if (!log) throw new Error(`Missing guardian event for ${name}`);
    const event = JSON.parse(log.slice(11));
    if (event.event !== expected) throw new Error(`Unexpected event for ${name}`);
    console.log(name, JSON.stringify(event), `https://explorer.testnet.near.org/transactions/${txHash(result)}`);
  }

  const spent = BigInt(await view('get_spent_today'));
  const expectedSpent = initialSpent + BigInt(CONFIG.amountOk);
  if (spent !== expectedSpent) throw new Error(`Wrong spent_today ${spent}, expected ${expectedSpent}`);
  console.log('Only the approved spend increased daily usage:', spent.toString());

  try {
    await account.transfer({ receiverId: CONFIG.vendor, amount: 1n });
    throw new Error('SECURITY FAILURE: direct transfer completed');
  } catch (error) {
    if (error.message.startsWith('SECURITY FAILURE:')) throw error;
    if (!isTransferBlocked(error)) throw error;
    console.log('Direct native NEAR transfer blocked:', error.name || error.type);
  }
  console.log('PASS: Allowed payment + over-limit + non-whitelist + protocol transfer block.');
}
main().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
