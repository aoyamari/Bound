# Testnet agent

`demo-agent.mjs` makes three real NEAR testnet transactions using an agent with a Function Call access key, then attempts one forbidden native NEAR transfer.

## Run (repeat with a funded agent, within the daily allowance)

```bash
cd scripts
npm install
cp .env.example .env.local
# Add AGENT_PRIVATE_KEY=ed25519:... to .env.local (ignored by git).
npm run demo
```

Default deployment and public settings are in `.env.example`. `AGENT_PRIVATE_KEY` is never included in the repository or frontend. Requires Node >=22.22.2 and NEAR SDK 7.4.0. Use `ed25519:`-prefixed secret strings in `.env.local`; no spaces. Alternatively `AGENT_KEY_FILE` may point to a NEAR credential JSON containing `private_key` or `secret_key`; keep that file outside the tracked repo. With an `ed25519:` prefix, SDK 7 accepts the secret.

The non-whitelist receiver `SCAM_ACCOUNT_ID` (only your verified testnet target) is never paid; the owner account is **not** a demo agent. `AMOUNT_OK=100000000000000000000000` = 0.1 NEAR; `AMOUNT_OVER_MAX=300000000000000000000000` = 0.3 NEAR (exceeds the 0.25 cap).

Expected flow: `SpendAllowed` (0.1), `SpendRejected / over_max_per_tx` (0.3), `SpendRejected / not_whitelisted` (0.1). The script reads daily spending before and after and proves only allowed payments consume budget. It prints explorer tx hashes and attempts a direct NEAR transfer using the same key; NEAR rejects direct transfer because FC keys cannot transfer.

After owner pauses: `npm run demo -- --paused-check`. For just the FC-key negative proof: `npm run demo -- --negative-only`.

Requires `near-api-js`. Unit tests: `cargo test --manifest-path ../contracts/agent-guardian/Cargo.toml`. Account setup is in the root README and `DEV.md`. Wasm hashing, without deploying, is `python3 contract-provenance.py --self-test` and `docs/DEPLOY.md`.

Immutable account budget is separate: FC allowance (if set) applies to gas, not the treasury limits. Demo keys are testnet-only; do not give an AI production full-access credentials.
