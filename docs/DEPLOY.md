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

The script confirms (executed and verified with near-api-js 7.4 on NEAR testnet):
- Allowed 0.1 NEAR transfer to whitelisted vendor;
- 0.3 NEAR rejected over-max;
- 0.1 NEAR to an unknown receiver rejected;
- `spent_today` correct before and after, not hardcoded;
- Native 1 yoctoNEAR transfer fails using the exact Function Call key.

Agent `--paused-check` expects owner already paused and will return success only on `reason=paused`. Restore owner unpause manually with CLI.

CLI pause/unpause:

```bash
near contract call-function as-transaction guardian.YOUR_NAME.testnet pause json-args '{}' \
  prepaid-gas '30 Tgas' attached-deposit '0 NEAR' sign-as YOUR_NAME.testnet \
  network-config testnet sign-with-keychain send
near contract call-function as-transaction guardian.YOUR_NAME.testnet unpause json-args '{}' \
  prepaid-gas '30 Tgas' attached-deposit '0 NEAR' sign-as YOUR_NAME.testnet \
  network-config testnet sign-with-keychain send
```

## Storage & test notes

Unit tests cover both ways the contract moves NEAR:

- `spend` creates a transfer receipt for the exact yocto amount when the policy allows. A rejected `spend` (pause, zero, whitelist, per-tx cap, daily cap, spendable balance) leaves `spent_today` unchanged and creates no transfer.
- `withdraw` is owner-only, rejects zero, rejects amounts above the spendable balance, and still works while paused. The receipt pays the owner and does not consume the daily cap.
- Non-owners cannot `set_policy`, whitelist add/remove, `pause`, `unpause`, or `withdraw`.
- `deposit` is not owner-gated. It only accepts a non-zero attached deposit.
- Amounts are integer yoctoNEAR. Views report the current UTC bucket immediately; the stored bucket rolls on the next `spend` or `set_policy`.
- Spendable balance reserves `storage_usage * 10^19` yoctoNEAR. That constant is `env::storage_byte_cost()` from near-sdk 5.17.2.

This is not a comprehensive audit. There is no owner-key rotation in the MVP. A full-access key on the guardian account can still change code or transfer NEAR outside these methods. `spend` does not restrict the predecessor, so any account can pay a whitelisted receiver up to the caps. No off-chain or TEE claims are made.

## Bytecode provenance

Live `guardian.bound-aoyamari.testnet` `view_code` hash is base58 `6SS3z8n8jBE2Co4pLGF6c24tAaWbi145PEtdvbZcLTh`. That is the standard base58 of SHA256 `0164a8f67c6087542cff3dfbb7404e95c07c971e05fb16adf245f386ec2046b0` (32 bytes, not base58check). The string `3882a4N4K5525P3Q7663C23U4A9BJ13375UDWB9C78H` does not encode that digest.

A read-only `contract_source_metadata` call returns:

```json
{"version":"0.1.0","link":"https://github.com/aoyamari/Bound","standards":[{"standard":"nep330","version":"1.3.0"}],"build_info":null}
```

`build_info` is null, so the wasm does not name a git commit. The link matches `Cargo.toml` `repository` from `469b74b` through `31ab724`. Those commits, including the documented deploy commit `05859e9`, have the same `lib.rs`, `Cargo.toml`, `Cargo.lock`, and `rust-toolchain.toml`. Earlier commits embed a different link (`https://github.com/aoyamari/bound` at `ec52f6a`, `https://github.com/example/onchain-agent-guardian` at `21f5987`) and are not this wasm.

Rust 1.88.0 (`6b00bc3880198600130e1cf62b8f8a93494488cc`, LLVM 20.1.5), command:

```bash
RUSTFLAGS='--remap-path-prefix=/usr/local/cargo=/home/box/.cargo' \
  cargo build --release --target wasm32-unknown-unknown --manifest-path contracts/agent-guardian/Cargo.toml
```

produced a 255,267-byte wasm whose SHA256 is `0164a8f67c6087542cff3dfbb7404e95c07c971e05fb16adf245f386ec2046b0`. The same command without that remap, with this machine's cargo home `/usr/local/cargo`, produced SHA256 `7c5d346b89f53d5f4023970520861f0f50e688d8a1e1817cbdcccb431fb67ab2`. The two files are the same length and differ only in 20 copies of that cargo-home prefix inside panic paths. No contract build workflow existed; `.github/workflows/pages.yml` at `cbca672` built the frontend only and was removed in `44de228`. Deploy transaction `GdqpWg8PvXefyA3Y4ShMo2d73fxFp5CaqovCs6MBWrmg` does not store a commit id.

This identifies the deployed source as that identical contract tree, not one commit inside it. It does not mean a later source change, another rustc, or a build with `NEP330_BUILD_INFO_*` set will match.

`scripts/contract-provenance.py` prints SHA256, base58, and the embedded NEP-330 JSON for a wasm. `.github/workflows/contract-provenance.yml` builds one commit with `build_info.source_code_snapshot` set to `HEAD` and cargo/workspace paths remapped to `/cargo-home` and `/workspace`. That artifact is for a future deploy. It is not the current testnet wasm, and the workflow does not deploy or publish Pages.

Optional read-only checks, with no key and no transaction:

```bash
curl -s https://rpc.testnet.near.org -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":"bound-view-code","method":"query","params":{"request_type":"view_code","finality":"final","account_id":"guardian.bound-aoyamari.testnet"}}' \
  | jq -r '.result.hash'
curl -s https://rpc.testnet.near.org -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":"bound-meta","method":"query","params":{"request_type":"call_function","finality":"final","account_id":"guardian.bound-aoyamari.testnet","method_name":"contract_source_metadata","args_base64":"e30="}}'
```

Agent key scope remains off-chain: `near account list-keys agent.bound-aoyamari.testnet network-config testnet now`.

### Frontend

React/Vite/TypeScript, public `get_policy`, `get_spent_today` and `can_spend` via `fetch`; no browser key. Owner wallet UI is not wired yet until specifically completed; not represented as live admin. The required Node version is >=22.22.2. For a supplied secret in NEAR account credential JSON, keep the key file outside the repo and pass `AGENT_KEY_FILE`.

## Dashboard publishing

Pages uses the static `gh-pages` branch. `.github/workflows/contract-provenance.yml` only builds and uploads a wasm artifact; it does not publish the dashboard. Maintain source on `main` and release compiled frontend assets with this user-entered command:

```bash
cd frontend
npm ci
npm run publish
```

`publish` runs tests, builds, then uses a dev-only `gh-pages` package to deploy `dist` to the `gh-pages` branch. A maintainer must configure repository Pages to `Deploy from a branch` / `gh-pages` / `/` and configure `https://<your-username>.github.io/<repo>/`. `frontend/vite.config.ts` has the `/Bound/` base. Browser RPC is public GET-style contract view through JSON-RPC. Direct contract mutation is CLI-only in MVP; the web app has no wallet or credentials.
