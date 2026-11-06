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

Default deployment and public settings are in `.env.example`. `AGENT_PRIVATE_KEY` is never included in the repository or frontend. Alternatively `AGENT_KEY_FILE` may point to an outside-repo NEAR credential JSON with a `private_key`.

The non-whitelist receiver `near-examples.testnet` is never paid; the owner account is **not** a demo agent. `AMOUNT_OK=100000000000000000000000` = 0.1 NEAR; `AMOUNT_OVER_MAX=300000000000000000000000` = 0.3 NEAR (exceeds the 0.25 cap).

Expected flow: `SpendAllowed` (0.1), `SpendRejected / over_max_per_tx` (0.3), `SpendRejected / not_whitelisted` (0.1). The script reads daily spending before and after and proves only allowed payments consume budget. It prints explorer tx hashes and runs `sendMoney` using the same key; NEAR rejects direct transfer because FC keys cannot transfer.

Requires `near-api-js`. Unit tests: `cargo test --manifest-path ../contracts/agent-guardian/Cargo.toml`. Reproducible accounts/deployment: root README and `DEV.md`.
