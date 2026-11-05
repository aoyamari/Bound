# Demo agent scripts

Node skeleton that exercises three `spend` scenarios against a deployed Guardian on NEAR testnet.

## Setup

```bash
cd scripts
cp .env.example .env   # if present; or export vars below
npm install
```

### Env vars

| Variable | Meaning |
|---|---|
| `NEAR_NETWORK` | `testnet` (default) |
| `GUARDIAN_CONTRACT` | e.g. `guardian.you.testnet` |
| `AGENT_ACCOUNT_ID` | account that holds the **FC key only** |
| `AGENT_PRIVATE_KEY` | ed25519 private key for that FC key (never commit) |
| `VENDOR_ACCOUNT_ID` | whitelisted receiver |
| `SCAM_ACCOUNT_ID` | non-whitelisted receiver for reject demo |
| `AMOUNT_OK` | yoctoNEAR under `max_per_tx` |
| `AMOUNT_OVER_MAX` | yoctoNEAR over `max_per_tx` |

## Proof to capture for judges

1. Allowed spend → `SpendAllowed` log.
2. Over max → `SpendRejected` / `over_max_per_tx`.
3. Bad receiver → `SpendRejected` / `not_whitelisted`.
4. (Manual) Attempt native transfer with the same FC key → fails at protocol level.

`demo-agent.mjs` still has TODOs for wiring credentials; fill env and complete the `near-api-js` connection blocks before the demo recording.
