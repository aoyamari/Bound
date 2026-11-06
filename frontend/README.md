# Bound dashboard

Polished read-only demo UI for **Bound** — *Autonomy, within bounds.*

Talks to the live NEAR testnet guardian via public RPC only. No backend. No wallet. No keys in the browser.

**Contract:** [`guardian.bound-aoyamari.testnet`](https://explorer.testnet.near.org/accounts/guardian.bound-aoyamari.testnet)
**RPC:** `https://rpc.testnet.near.org`
**Views used:** `get_policy`, `get_spent_today`, `can_spend`

## Run

```bash
cd frontend
npm install
npm run dev
```

Open **http://localhost:5173** (Vite binds `0.0.0.0:5173`).

Other scripts:

```bash
npm test          # unit tests (yocto helpers)
npm run build     # production build → dist/
npm run preview   # serve dist on :5173
```

## What you see

- Giant typographic hero + **LIVE NEAR TESTNET** badge and explorer deep-link
- Three live stat cards: max per payment, daily allowance, spent today
- Architecture / live-simulate focus toggle
- Three preconfigured dry-runs (real `can_spend` results):
  1. `0.1 Ⓝ` → `vendor.bound-aoyamari.testnet` (expect allow)
  2. `0.3 Ⓝ` → same vendor (expect `over_max_per_tx`; policy max is `0.25 Ⓝ`)
  3. `0.1 Ⓝ` → `near-examples.testnet` (expect `not_whitelisted`)

## Stack

React 18 + Vite + TypeScript. Raw `fetch` to NEAR JSON-RPC — no `near-api-js`, no secrets.

## Publish

`npm run publish` test/builds and publishes `dist` to the repository's `gh-pages` branch. Do not publish during on-chain owner-admin operations; owner controls are only through documented NEAR CLI in this MVP.
