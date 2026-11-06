# Reproduce Bound on NEAR testnet

These are the actual commands used for **guardian.bound-aoyamari.testnet**, with replaceable placeholders. Use **your own account** for all `YOUR_NAME.testnet` mentions. All accounts and funds here are testnet.

## Build

Rust 1.88+, wasm32-unknown-unknown, and `npm install -g near-cli-rs@latest`.

```bash
cargo test --manifest-path contracts/agent-guardian/Cargo.toml
rustup target add wasm32-unknown-unknown --toolchain 1.88.0
cargo build --release --target wasm32-unknown-unknown --manifest-path contracts/agent-guardian/Cargo.toml
```

## Accounts

```bash
near create-account YOUR_NAME.testnet --use-faucet --network-id testnet
near create-account guardian.YOUR_NAME.testnet --use-account YOUR_NAME.testnet --initial-balance 5 --network-id testnet
near create-account agent.YOUR_NAME.testnet --use-account YOUR_NAME.testnet --initial-balance 1 --network-id testnet
near create-account vendor.YOUR_NAME.testnet --use-account YOUR_NAME.testnet --initial-balance 0.1 --network-id testnet
```

## Deploy, initialize, whitelist

Use supported current `near-cli-rs` syntax (CLI changes flags between versions). Example:

```bash
near contract deploy guardian.YOUR_NAME.testnet use-file contracts/agent-guardian/target/wasm32-unknown-unknown/release/agent_guardian.wasm \
  with-init-call new json-args '{"owner":"YOUR_NAME.testnet","max_per_tx":"250000000000000000000000","daily_limit":"500000000000000000000000"}' \
  prepaid-gas '100 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-keychain send

near contract call-function as-transaction guardian.YOUR_NAME.testnet add_to_whitelist \
  json-args '{"account_id":"vendor.YOUR_NAME.testnet"}' prepaid-gas '30 Tgas' attached-deposit '0 NEAR' \
  sign-as YOUR_NAME.testnet network-config testnet sign-with-keychain send

near contract call-function as-read-only guardian.YOUR_NAME.testnet get_policy json-args '{}' network-config testnet now
```

## Agent access key

`near account add-key grant-function-call-access` might require a TTY (best used interactively). For a headless supported SDK path, generate a new `ed25519` key with `KeyPair.fromRandom` in `near-api-js` and store it in ignored local credentials. With the **full access bootstrap key still in the account**:

```js
await account.addFunctionCallAccessKey({ publicKey: generatedPublicKey, contractId: 'guardian.YOUR_NAME.testnet', methodNames: ['spend'] })
await account.deleteKey(bootstrapPublicKey)
console.log(await account.getAccessKeys())
```

Afterwards the agent account should have a single Function Call permission restricted to the guardian and `spend`; remove revoked bootstrap credential files as well. Do not add your real user's NEAR keys to the agent's account or send them in chat.

Set `OWNER_ACCOUNT_ID` and `OWNER_PRIVATE_KEY` locally to use owner admin calls. Add `owner-id` and an admin script before browser wallet signing; until then, CLI owner controls are the implemented admin path.

## Demo

```bash
cd scripts
npm install
cp .env.example .env.local
# Modify guardian/agent/vendor IDs; put agent secret only in .env.local.
npm run demo
npm run demo -- --negative-only
# Owner must pause first before this single-request phase.
npm run demo -- --paused-check
```

The script confirms:
- Allowed 0.1 NEAR transfer to whitelisted vendor;
- 0.3 NEAR rejected over-max;
- 0.1 NEAR to an unknown receiver rejected;
- `spent_today` correct before and after, not hardcoded;
- Native 1 yoctoNEAR transfer fails using the exact Function Call key.

Agent `--paused-check` expects owner already paused and will return success only on `reason=paused`. Restore owner unpause manually with CLI; unsigned browser owner forms must not claim any mutation happened.

## Storage & test notes

Current contract has no unit-test withdrawal check; coverage is explicitly not a comprehensive audit. UTC bucket increments lazily on next mutation. No owner-key rotation in MVP; production requires deliberate owner transfer/multisig controls. Owner can withdraw outside agent policy; that is intentional. No Off-chain or TEE claims are made.

### Frontend

React/Vite/TypeScript, public `get_policy`, `get_spent_today` and `can_spend` via `fetch`; no browser key. Owner wallet UI is not wired yet until specifically completed; not represented as live admin. The required Node version is >=22.22.2. For a supplied secret in NEAR account credential JSON, keep the key file outside the repo and pass `AGENT_KEY_FILE`.

## Dashboard publishing

This repo uses the static `gh-pages` deployment branch (not `.github/workflows`). Maintain source on `main` and release compiled assets with this user-entered command:

```bash
cd frontend
npm ci
npm run publish
```

`publish` runs tests, builds, then uses a dev-only `gh-pages` package to deploy `dist` to the `gh-pages` branch. A maintainer must configure repository Pages to `Deploy from a branch` / `gh-pages` / `/` and configure `https://<your-username>.github.io/<repo>/`. `frontend/vite.config.ts` has the `/Bound/` base. Browser RPC is public GET-style contract view through JSON-RPC. Direct contract mutation is CLI-only in MVP; the web app has no wallet or credentials.
