# Bound development and live proof

**Test only:** all accounts below are on NEAR **testnet**. Never use its private keys on mainnet.

## Live deployment

- Owner `bound-aoyamari.testnet`
- Guardian `guardian.bound-aoyamari.testnet`
- Agent `agent.bound-aoyamari.testnet` (only `spend` on guardian permitted)
- Whitelist `vendor.bound-aoyamari.testnet`
- Config: 0.25 NEAR max payment / 0.5 NEAR per UTC day
- Testnet treasury ~5 NEAR; deploy [transaction](https://explorer.testnet.near.org/transactions/GdqpWg8PvXefyA3Y4ShMo2d73fxFp5CaqovCs6MBWrmg). That transaction id is not a git commit. The wasm's NEP-330 `build_info` is null. See `docs/DEPLOY.md` for the source range and cargo-path check.
- Add whitelist [transaction](https://explorer.testnet.near.org/transactions/4DfDaNBXAKtPHuzATaMnC6b3r6E2G1QWZSBB8FvgrtZh)
- Added Function Call key [transaction](https://explorer.testnet.near.org/transactions/EodNEt68piFniETwQLxMKRaJDNK5bR5o6DDtreXEY639)
- Deleted full-access bootstrap key [transaction](https://explorer.testnet.near.org/transactions/aWtyzmteNMx9Ss3y5f5gF1kC5CzMfQC2uqWeqDZQd7f)

All guardian private credentials stay outside tracked files, locally; full-access owner credentials are outside the repo, while the restricted agent credential is under ignored `scripts/credentials/`. `.gitignore` excludes credentials and local env files. None of those files or raw private keys belongs in the public repo, browser, or demo.

## Verified live receipts (2026-11-06)

- **Allowed 0.1 NEAR:** [`3pNHTman...`](https://explorer.testnet.near.org/transactions/3pNHTmanHQ8gPtnQNVeCLt7k4YuemMYNc9aTL6NtYoqR) -> `SpendAllowed`, `memo=bound-allowed`, daily spend 0.1 -> 0.2 NEAR.
- **Over limit 0.3 NEAR:** [`B8p9RA7z...`](https://explorer.testnet.near.org/transactions/B8p9RA7zqgARfrJ6g1k1mrPP1Mx63urtPfJ3po6V1Z4x) -> `SpendRejected: over_max_per_tx`.
- **Not whitelisted 0.1 NEAR:** [`CbaJYocR...`](https://explorer.testnet.near.org/transactions/CbaJYocRDNseff9iAtpchbtD8o6pcy6WvHQxssL3Mbn9) -> `SpendRejected: not_whitelisted`.

Direct native transfer of 1 yoctoNEAR using the same agent key was attempted before these runs and rejected `RequiresFullAccess` by protocol. No treasury transfer to the intended recipient succeeded. `npm run demo` includes the same negative test. The demo script reads the current daily usage first, verifies the relative allowed increment, and refuses when daily headroom is insufficient. This preserves accounting when invoked more than once on different days.

## Owner emergency stop

- [Pause](https://explorer.testnet.near.org/transactions/DYAbRR83n3QRCYnFp8Ft3REGQSt9MrcygThDv7VDwA4B)
- [Unpause](https://explorer.testnet.near.org/transactions/EzukxiKFGnP64zQkeozVwdNhZxHTNqJjXdwvXpkq4WDS)
- While paused, legitimate whitelisted spending emitted `SpendRejected / paused`; use `npm run demo -- --paused-check` to assert it. Treasury was restored active.

## Static dashboard hosting

GitHub Pages builds `frontend/` and publishes the client-only dashboard. RPC is public read access; no keys enter the browser.

## State reads

```bash
near contract call-function as-read-only guardian.bound-aoyamari.testnet get_policy json-args '{}' network-config testnet now
near contract call-function as-read-only guardian.bound-aoyamari.testnet can_spend json-args '{"receiver":"vendor.bound-aoyamari.testnet","amount":"300000000000000000000000"}' network-config testnet now
near account list-keys agent.bound-aoyamari.testnet network-config testnet now
```


### Revalidation with near-api-js 7.4.0

The demo now uses supported SDK 7.4.0 (`Account`, `KeyPairSigner`, explicit bigint gas and deposit). `npm audit` reports zero production vulnerabilities. Old v5 `connect`/`keyStores` APIs are no longer required. If using an external credential key JSON, `KeyPairSigner.fromSecretKey` expects the `ed25519:` prefix.

Latest validated end-to-end demo:
- [Allowed 0.1 NEAR](https://explorer.testnet.near.org/transactions/BuuwybQdwNjbqNt4hgxp5ssFkePr3REQQtNCvURKAMdC) -> accumulated day usage exactly 0.5 NEAR.
- [Over-max rejection](https://explorer.testnet.near.org/transactions/3cL4xH9gBxbLdoc7678rJqJYMHMNAprBZitcJeVtd1Z7)
- [Non-whitelisted rejection](https://explorer.testnet.near.org/transactions/7RYBvjzyTin3w7YmUvsNp2bLswwq51Kqp8AQaNc8e66W)
- [Agent request during pause](https://explorer.testnet.near.org/transactions/EthpUMqJJXzMW6rsTiFNDm9zwbmRHv65xRiZQ489QkAp) -> `SpendRejected: paused`.
- Restored [unpause](https://explorer.testnet.near.org/transactions/78nWR2WyuQip5djUujJVjXmMxXkAJJFqXMApdU7dNH6E) after pause proof.

Do not repeat the positive scenario after the cap is exhausted: default script safely refuses once configured allowance is reached. Dry-run cards remain available until the UTC reset. Negative-only and paused checks do not move whitelist treasury funds.
