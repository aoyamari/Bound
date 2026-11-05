# Frontend (placeholder)

Minimal owner UI for On-chain Agent Guardian. Not implemented yet — build this after the contract tests are green and testnet deploy works.

## What the UI should do

1. **Connect owner wallet** (NEAR testnet).
2. **Policy editor**
   - Show `get_policy` (max_per_tx, daily_limit, paused, whitelist).
   - Forms: `set_policy`, `add_to_whitelist` / `remove_from_whitelist`, `pause` / `unpause`.
   - Deposit / top-up treasury; optional `withdraw`.
3. **Allow / reject feed**
   - Tail contract logs / indexer events for `SpendAllowed` and `SpendRejected`.
   - Display reason strings (`over_max_per_tx`, `not_whitelisted`, `over_daily_limit`, `paused`, …).
4. **Badge:** “Policy is public on-chain” + link to explorer contract + `get_policy` call.
5. **Demo helper:** buttons that call `can_spend` dry-run before sending `spend` from a separate agent key (agent key should not live in the owner browser session).

## Suggested stack

- Next.js + `@near-wallet-selector` + `near-api-js`, or a single static HTML page for the hackathon MVP.

## Non-goals

- Hosting agent private keys in the browser.
- Cloning OutLayer’s TEE / encrypted-policy UX.
