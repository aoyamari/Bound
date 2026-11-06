# Bound development and live proof

**Test only:** all accounts below are on NEAR **testnet**. Never use its private keys on mainnet.

## Live deployment

- Owner `bound-aoyamari.testnet`
- Guardian `guardian.bound-aoyamari.testnet`
- Agent `agent.bound-aoyamari.testnet` (only `spend` on guardian permitted)
- Whitelist `vendor.bound-aoyamari.testnet`
- Config: 0.25 NEAR max payment / 0.5 NEAR per UTC day
- Testnet treasury ~5 NEAR; deploy [transaction](https://explorer.testnet.near.org/transactions/GdqpWg8PvXefyA3Y4ShMo2d73fxFp5CaqovCs6MBWrmg)
- Add whitelist [transaction](https://explorer.testnet.near.org/transactions/4DfDaNBXAKtPHuzATaMnC6b3r6E2G1QWZSBB8FvgrtZh)
- Added Function Call key [transaction](https://explorer.testnet.near.org/transactions/EodNEt68piFniETwQLxMKRaJDNK5bR5o6DDtreXEY639)
- Deleted full-access bootstrap key [transaction](https://explorer.testnet.near.org/transactions/aWtyzmteNMx9Ss3y5f5gF1kC5CzMfQC2uqWeqDZQd7f)

All guardian private credentials stay outside tracked files, in `/home/box/.near-credentials/testnet/` on the development machine. `.gitignore` also excludes `credentials/`. None of those files or raw private keys belongs in the public repo, browser, or demo.

## Verified live receipts (2026-11-06)

- **Allowed 0.1 NEAR:** [`3pNHTman...`](https://explorer.testnet.near.org/transactions/3pNHTmanHQ8gPtnQNVeCLt7k4YuemMYNc9aTL6NtYoqR) -> `SpendAllowed`, `memo=bound-allowed`, daily spend 0.1 -> 0.2 NEAR.
- **Over limit 0.3 NEAR:** [`B8p9RA7z...`](https://explorer.testnet.near.org/transactions/B8p9RA7zqgARfrJ6g1k1mrPP1Mx63urtPfJ3po6V1Z4x) -> `SpendRejected: over_max_per_tx`.
- **Not whitelisted 0.1 NEAR:** [`CbaJYocR...`](https://explorer.testnet.near.org/transactions/CbaJYocRDNseff9iAtpchbtD8o6pcy6WvHQxssL3Mbn9) -> `SpendRejected: not_whitelisted`.

Direct native transfer of 1 yoctoNEAR using the same agent key was attempted before these runs and rejected `RequiresFullAccess` by protocol. No treasury transfer to the intended recipient succeeded. `npm run demo` includes the same negative test. For the demo script (relative daily increment is validated), read `get_spent_today` before the run to ensure headroom; don't invent a fresh account counter.

## State reads

```bash
near contract call-function as-read-only guardian.bound-aoyamari.testnet get_policy json-args '{}' network-config testnet now
near contract call-function as-read-only guardian.bound-aoyamari.testnet can_spend json-args '{"receiver":"vendor.bound-aoyamari.testnet","amount":"300000000000000000000000"}' network-config testnet now
near account list-keys agent.bound-aoyamari.testnet network-config testnet now
```
