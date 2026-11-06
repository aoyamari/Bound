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
  const { connect, keyStores, KeyPair } = await import('near-api-js');
  const keyStore = new keyStores.InMemoryKeyStore();
  const keyPath = process.env.AGENT_KEY_FILE || resolve(__dirname, 'credentials/agent-fc-key.json');
  if (process.env.AGENT_PRIVATE_KEY) {
    await keyStore.setKey(CONFIG.network, CONFIG.agentId, KeyPair.fromString(process.env.AGENT_PRIVATE_KEY));
  } else {
    const k = JSON.parse(readFileSync(keyPath, 'utf8'));
    await keyStore.setKey(CONFIG.network, CONFIG.agentId, KeyPair.fromString(k.private_key || k.secret_key));
  }
  const account = await (await connect({ networkId: CONFIG.network, keyStore, nodeUrl: 'https://rpc.testnet.near.org' })).account(CONFIG.agentId);
  console.log('Bound |', CONFIG.network, '|', CONFIG.agentId);
  console.log('Public policy:', await account.viewFunction({ contractId: CONFIG.contract, methodName: 'get_policy', args: {} }));
  if (process.argv.includes('--paused-check')) {
    const result = await account.functionCall({ contractId: CONFIG.contract, methodName: 'spend', args: { receiver: CONFIG.vendor, amount: CONFIG.amountOk, memo: 'paused-check' }, gas: '30000000000000', attachedDeposit: '0' });
    const log = (result.receipts_outcome || []).flatMap(x => x.outcome.logs || []).find(x => x.includes('SpendRejected'));
    if (!log || !log.includes('"reason":"paused"')) throw new Error(`Expected paused rejection; owner must pause first. ${log || ''}`);
    console.log(`PASS: paused request rejected, no transfer: https://explorer.testnet.near.org/transactions/${result.transaction.hash}`);
    return;
  }
  if (process.argv.includes('--negative-only')) {
    await account.sendMoney(CONFIG.vendor, '1').then(() => { throw new Error('SECURITY FAILURE: direct transfer completed'); }).catch(error => {
      if (['RequiresFullAccess', 'TransactionError', 'InvalidTxError', 'UntypedError'].includes(error.type)) {
        console.log('PASS: native NEAR transfer blocked by Function Call key:', error.type);
        return;
      }
      throw error;
    });
    return;
  }
  const initialSpent = BigInt(await account.viewFunction({ contractId: CONFIG.contract, methodName: 'get_spent_today', args: {} }));
  const daily = BigInt((await account.viewFunction({ contractId: CONFIG.contract, methodName: 'get_policy', args: {} })).daily_limit);
  if (initialSpent + BigInt(CONFIG.amountOk) > daily) throw new Error('Insufficient daily headroom. Rerun after the UTC reset or configure a higher budget.');

  for (const [name, receiver, amount, expected] of [
    ['allowed', CONFIG.vendor, CONFIG.amountOk, 'SpendAllowed'],
    ['over-max', CONFIG.vendor, CONFIG.amountOverMax, 'SpendRejected'],
    ['not-whitelisted', CONFIG.scam, CONFIG.amountOk, 'SpendRejected'],
  ]) {
    const result = await account.functionCall({ contractId: CONFIG.contract, methodName: 'spend', args: { receiver, amount, memo: `bound-${name}` }, gas: '30000000000000', attachedDeposit: '0' });
    const log = (result.receipts_outcome || []).flatMap(x => x.outcome.logs || []).find(x => x.startsWith('EVENT_JSON:'));
    if (!log) throw new Error(`Missing guardian event for ${name}`);
    const event = JSON.parse(log.slice(11));
    if (event.event !== expected) throw new Error(`Unexpected event for ${name}`);
    console.log(name, JSON.stringify(event), `https://explorer.testnet.near.org/transactions/${result.transaction.hash}`);
  }

  const spent = BigInt(await account.viewFunction({ contractId: CONFIG.contract, methodName: 'get_spent_today', args: {} }));
  const expectedSpent = initialSpent + BigInt(CONFIG.amountOk);
  if (spent !== expectedSpent) throw new Error(`Wrong spent_today ${spent}, expected ${expectedSpent}`);
  console.log('Only the approved spend increased daily usage:', spent.toString());

  try {
    await account.sendMoney(CONFIG.vendor, '1');
    throw new Error('SECURITY FAILURE: direct transfer completed');
  } catch (error) {
    if (error.message.startsWith('SECURITY FAILURE:')) throw error;
    if (!['RequiresFullAccess', 'TransactionError', 'InvalidTxError', 'UntypedError'].includes(error.type)) throw error;
    console.log('Direct native NEAR transfer blocked:', error.type);
  }
  console.log('PASS: Allowed payment + over-limit + non-whitelist + protocol transfer block.');
}
main().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
