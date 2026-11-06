#!/usr/bin/env node
/**
 * Bound — demo agent skeleton
 *
 * Goal: call guardian.spend with an account that should ONLY have a
 * Function Call access key scoped to method `spend`.
 *
 * Scenarios:
 *   1) Allowed — whitelisted vendor, amount <= max_per_tx & daily
 *   2) Rejected — amount > max_per_tx
 *   3) Rejected — receiver not whitelisted
 *
 * Extra (manual / separate script): prove the same key cannot native-transfer NEAR.
 *
 * Fill env vars (see README / .env.example). Do not commit private keys.
 */

import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));

function loadDotEnv() {
  const p = resolve(__dirname, '.env');
  if (!existsSync(p)) return;
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

loadDotEnv();

const CONFIG = {
  network: process.env.NEAR_NETWORK || 'testnet',
  contract: process.env.GUARDIAN_CONTRACT || '',
  agentId: process.env.AGENT_ACCOUNT_ID || '',
  agentKey: process.env.AGENT_PRIVATE_KEY || '',
  vendor: process.env.VENDOR_ACCOUNT_ID || 'vendor.testnet',
  scam: process.env.SCAM_ACCOUNT_ID || 'scam.testnet',
  amountOk: process.env.AMOUNT_OK || '100000000000000000000000', // 0.1 NEAR
  amountOverMax: process.env.AMOUNT_OVER_MAX || '2000000000000000000000000', // 2 NEAR
};

function requireConfig() {
  const missing = [];
  if (!CONFIG.contract) missing.push('GUARDIAN_CONTRACT');
  if (!CONFIG.agentId) missing.push('AGENT_ACCOUNT_ID');
  if (!CONFIG.agentKey) missing.push('AGENT_PRIVATE_KEY');
  if (missing.length) {
    console.error('Missing env:', missing.join(', '));
    console.error('Copy scripts/.env.example → scripts/.env and fill values.');
    console.error('Also: npm install  # pulls near-api-js');
    process.exit(1);
  }
}

/**
 * TODO: wire near-api-js once credentials exist.
 *
 * Suggested shape (near-api-js v5+ — verify against installed docs):
 *
 *   import { connect, keyStores, KeyPair } from 'near-api-js';
 *   const keyStore = new keyStores.InMemoryKeyStore();
 *   await keyStore.setKey(CONFIG.network, CONFIG.agentId, KeyPair.fromString(CONFIG.agentKey));
 *   const near = await connect({ networkId: CONFIG.network, keyStore, nodeUrl: ... });
 *   const account = await near.account(CONFIG.agentId);
 *   await account.functionCall({
 *     contractId: CONFIG.contract,
 *     methodName: 'spend',
 *     args: { receiver, amount, memo },
 *     gas: '30000000000000',
 *     attachedDeposit: '0',
 *   });
 */
async function callSpend(receiver, amountYocto, memo) {
  console.log(`→ spend({ receiver: ${receiver}, amount: ${amountYocto}, memo: ${memo} })`);
  // TODO: replace with real functionCall (see comment above).
  throw new Error(
    'TODO: connect near-api-js and call spend. Scaffold only — see scripts/README.md',
  );
}

async function scenario(name, fn) {
  console.log('\n===', name, '===');
  try {
    await fn();
    console.log('Submitted. Check tx logs for SpendAllowed / SpendRejected.');
  } catch (e) {
    console.error('Scenario error:', e.message || e);
  }
}

async function main() {
  console.log('Bound — demo agent');
  console.log('Network:', CONFIG.network);
  requireConfig();
  console.log('Contract:', CONFIG.contract);
  console.log('Agent:', CONFIG.agentId);

  // Dynamic import so `node demo-agent.mjs` can still print help before npm install.
  try {
    await import('near-api-js');
    console.log('near-api-js: available (wire TODO in callSpend)');
  } catch {
    console.warn('near-api-js not installed yet — run: npm install');
  }

  await scenario('1) ALLOW — whitelisted vendor, under max_per_tx', async () => {
    await callSpend(CONFIG.vendor, CONFIG.amountOk, 'demo-allow');
  });

  await scenario('2) REJECT — over max_per_tx', async () => {
    await callSpend(CONFIG.vendor, CONFIG.amountOverMax, 'demo-over-max');
  });

  await scenario('3) REJECT — not whitelisted', async () => {
    await callSpend(CONFIG.scam, CONFIG.amountOk, 'demo-scam');
  });

  console.log(`
Next (manual FC-key proof):
  With the SAME Function Call key, try a native transfer of 1 yoctoNEAR.
  It must fail: FC keys cannot Transfer. That is the Guardian trust story.
`);
}

main();
