# Bound development and live proof

**Test only:** all accounts below are on NEAR **testnet**. Never use its private keys on mainnet.

## Live deployment

- Owner `bound-aoyamari.testnet`
- Guardian `guardian.bound-aoyamari.testnet`
- Agent `agent.bound-aoyamari.testnet` (only `spend` on guardian permitted)
- Whitelist `vendor.bound-aoyamari.testnet`
- Config: 0.25 NEAR max payment / 0.5 NEAR per UTC day
- Testnet treasury ~5 NEAR; first deploy [transaction](https://explorer.testnet.near.org/transactions/GdqpWg8PvXefyA3Y4ShMo2d73fxFp5CaqovCs6MBWrmg). That transaction id is not a git commit. See `docs/DEPLOY.md` for the first wasm's source range.
- Add whitelist [transaction](https://explorer.testnet.near.org/transactions/4DfDaNBXAKtPHuzATaMnC6b3r6E2G1QWZSBB8FvgrtZh)
- Added Function Call key [transaction](https://explorer.testnet.near.org/transactions/EodNEt68piFniETwQLxMKRaJDNK5bR5o6DDtreXEY639)
- Deleted full-access bootstrap key [transaction](https://explorer.testnet.near.org/transactions/aWtyzmteNMx9Ss3y5f5gF1kC5CzMfQC2uqWeqDZQd7f)

All guardian private credentials stay outside tracked files, locally; full-access owner credentials are outside the repo, while the restricted agent credential is under ignored `scripts/credentials/`. `.gitignore` excludes credentials and local env files. None of those files or raw private keys belongs in the public repo, browser, or demo.

## Code upgrade (testnet, state kept)

Second deploy, main `9e585567f69163f1e9bbbc4d53e35e9a9901983c`, [`Cb6saE7z...`](https://explorer.testnet.near.org/transactions/Cb6saE7zu8kRQUYTqqhjgmNpzooTTLJsov7FEvy4HCwH). Read-only tx status: signer and receiver `guardian.bound-aoyamari.testnet`, one `DeployContract` action, `SuccessValue` (`without-init-call`). This note does not name a compiler for that wasm.

Read-only `view_code` (2026-11-06): base58 `FagUutviDcjAKZY6Crv8Aj7H3T41jkqBFhuH1kCGWf3m`, SHA256 `d8a46cdd310339fb0c9654eacdc7879894e0663038a64039e684b1d72d733110`, 255,688 bytes. `contract_source_metadata` is still `{"version":"0.1.0","link":"https://github.com/aoyamari/Bound",...,"build_info":null}`. The bytes do not carry a git commit. `.github/workflows/contract-provenance.yml` can set `NEP330_BUILD_INFO_*` on a later build; it was not used for this deploy, and this follow-up does not change the contract.

`get_policy` after the upgrade: owner `bound-aoyamari.testnet`, max `250000000000000000000000` (0.25 NEAR), daily `500000000000000000000000` (0.5 NEAR), `paused` false, whitelist only `vendor.bound-aoyamari.testnet`.

`get_spent_today` on UTC day bucket `20763` (2026-11-06) is `500000000000000000000000` (0.5 / 0.5). No new positive payment has landed on this wasm. Earlier `SpendAllowed` receipts are the previous bytecode. New `spend` receipts for over-max and a non-whitelisted receiver are still pending: `demo-agent.mjs` stops when the daily cap is full, and that script was not changed here.

Read-only `can_spend` after the upgrade:

- `vendor.bound-aoyamari.testnet` 0.3 NEAR → `over_max_per_tx`
- `near-examples.testnet` 0.1 NEAR → `not_whitelisted`
- `vendor.bound-aoyamari.testnet` 0.1 NEAR → `over_daily_limit`

Pause on the upgraded code, then restored:

- Owner [pause](https://explorer.testnet.near.org/transactions/7uUR1gfap394ABxjmsieZgEQWXa31zMSCgkoi1MVnsF5) → `Paused`
- Agent [spend 0.1 NEAR to vendor](https://explorer.testnet.near.org/transactions/DBoVGpNmUkLBX4M7Xars3JtmUuw9gASv5RMkeG1H3EEe) → `SpendRejected` / `paused` (no transfer)
- Owner [unpause](https://explorer.testnet.near.org/transactions/G7HnvG3fezT7jeWuzAWaAWn2EVw4Yx3JFumntZ8TZaD4) → `Unpaused`

`view_access_key_list` for `agent.bound-aoyamari.testnet` returns one key, `ed25519:7MMphMSA1J6R9w9kWL2RJmj3nvC8P3T32dDYy2TyExA3`, Function Call to `guardian.bound-aoyamari.testnet`, method `spend` only. A direct NEAR transfer with that key was rejected `AccessKeyRequiresFullAccessActionError`. This follow-up did not submit that transfer again.

## Verified live receipts on the first bytecode (2026-11-06)

- **Allowed 0.1 NEAR:** [`3pNHTman...`](https://explorer.testnet.near.org/transactions/3pNHTmanHQ8gPtnQNVeCLt7k4YuemMYNc9aTL6NtYoqR) -> `SpendAllowed`, `memo=bound-allowed`, daily spend 0.1 -> 0.2 NEAR.
- **Over limit 0.3 NEAR:** [`B8p9RA7z...`](https://explorer.testnet.near.org/transactions/B8p9RA7zqgARfrJ6g1k1mrPP1Mx63urtPfJ3po6V1Z4x) -> `SpendRejected: over_max_per_tx`.
- **Not whitelisted 0.1 NEAR:** [`CbaJYocR...`](https://explorer.testnet.near.org/transactions/CbaJYocRDNseff9iAtpchbtD8o6pcy6WvHQxssL3Mbn9) -> `SpendRejected: not_whitelisted`.

Direct native transfer of 1 yoctoNEAR using the same agent key was attempted before these runs and rejected `RequiresFullAccess` by protocol. No treasury transfer to the intended recipient succeeded. `npm run demo` includes the same negative test. The demo script reads the current daily usage first, verifies the relative allowed increment, and refuses when daily headroom is insufficient. This preserves accounting when invoked more than once on different days.

## Owner emergency stop on the first bytecode

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

Latest end-to-end demo on the first bytecode (these receipts are not the upgraded wasm):
- [Allowed 0.1 NEAR](https://explorer.testnet.near.org/transactions/BuuwybQdwNjbqNt4hgxp5ssFkePr3REQQtNCvURKAMdC) -> accumulated day usage exactly 0.5 NEAR.
- [Over-max rejection](https://explorer.testnet.near.org/transactions/3cL4xH9gBxbLdoc7678rJqJYMHMNAprBZitcJeVtd1Z7)
- [Non-whitelisted rejection](https://explorer.testnet.near.org/transactions/7RYBvjzyTin3w7YmUvsNp2bLswwq51Kqp8AQaNc8e66W)
- [Agent request during pause](https://explorer.testnet.near.org/transactions/EthpUMqJJXzMW6rsTiFNDm9zwbmRHv65xRiZQ489QkAp) -> `SpendRejected: paused`.
- Restored [unpause](https://explorer.testnet.near.org/transactions/78nWR2WyuQip5djUujJVjXmMxXkAJJFqXMApdU7dNH6E) after pause proof.

Do not repeat the positive scenario after the cap is exhausted: default script safely refuses once configured allowance is reached. Dry-run cards remain available until the UTC reset. Negative-only and paused checks do not move whitelist treasury funds.
