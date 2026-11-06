//! Bound — exact, public spending policy for AI agents on NEAR.
//!
//! Funds live in this contract. Agents should only hold a Function Call access key
//! scoped to `spend`. Policy (limits + whitelist) is readable on-chain by anyone.

use near_sdk::borsh::BorshSerialize;
use near_sdk::json_types::U128;
use near_sdk::store::IterableSet;
use near_sdk::{
    env, near, require, AccountId, BorshStorageKey, NearToken, PanicOnDefault, Promise,
};

/// Nanoseconds in one UTC day.
const NS_PER_DAY: u64 = 86_400_000_000_000;

#[derive(BorshStorageKey, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    Whitelist,
}

/// Public policy snapshot (view-friendly).
#[derive(Clone, Debug, PartialEq, Eq)]
#[near(serializers = [json, borsh])]
pub struct PolicyView {
    pub owner: AccountId,
    pub max_per_tx: U128,
    pub daily_limit: U128,
    pub paused: bool,
    pub whitelist: Vec<AccountId>,
}

/// Result of a dry-run spend check.
#[derive(Clone, Debug, PartialEq, Eq)]
#[near(serializers = [json, borsh])]
pub struct CanSpendView {
    pub allowed: bool,
    pub reason: String,
    pub spent_today: U128,
    pub remaining_daily: U128,
    pub day_bucket: u64,
}

#[derive(PanicOnDefault)]
#[near(contract_state)]
pub struct Contract {
    owner: AccountId,
    /// Max yoctoNEAR allowed in a single `spend` call.
    max_per_tx: u128,
    /// Max yoctoNEAR that may leave the treasury per UTC day bucket.
    daily_limit: u128,
    /// YoctoNEAR already spent in `day_bucket`.
    spent_today: u128,
    /// `floor(block_timestamp_ns / NS_PER_DAY)`.
    day_bucket: u64,
    paused: bool,
    whitelist: IterableSet<AccountId>,
}

#[near]
impl Contract {
    /// Deploy + initialize. Caller becomes owner. Limits are in yoctoNEAR.
    #[init]
    pub fn new(owner: AccountId, max_per_tx: U128, daily_limit: U128) -> Self {
        require!(max_per_tx.0 > 0, "max_per_tx must be > 0");
        require!(daily_limit.0 > 0, "daily_limit must be > 0");
        require!(
            daily_limit.0 >= max_per_tx.0,
            "daily_limit must be >= max_per_tx"
        );
        Self {
            owner,
            max_per_tx: max_per_tx.0,
            daily_limit: daily_limit.0,
            spent_today: 0,
            day_bucket: current_day_bucket(),
            paused: false,
            whitelist: IterableSet::new(StorageKey::Whitelist),
        }
    }

    // ─── Owner admin ─────────────────────────────────────────────────────────

    #[payable]
    pub fn deposit(&mut self) {
        // Payable no-op: attached NEAR stays on the contract account (treasury).
        let attached = env::attached_deposit();
        require!(!attached.is_zero(), "attach NEAR to deposit");
        env::log_str(&format!(
            "EVENT_JSON:{{\"standard\":\"agent-guardian\",\"version\":\"1.0.0\",\"event\":\"Deposit\",\"data\":{{\"amount\":\"{}\"}}}}",
            attached.as_yoctonear()
        ));
    }

    pub fn set_policy(&mut self, max_per_tx: U128, daily_limit: U128) {
        self.assert_owner();
        require!(max_per_tx.0 > 0, "max_per_tx must be > 0");
        require!(daily_limit.0 > 0, "daily_limit must be > 0");
        require!(
            daily_limit.0 >= max_per_tx.0,
            "daily_limit must be >= max_per_tx"
        );
        self.roll_day_if_needed();
        self.max_per_tx = max_per_tx.0;
        self.daily_limit = daily_limit.0;
        // If new daily_limit is below spent_today, further spends fail until next day.
        env::log_str(&format!(
            "EVENT_JSON:{{\"standard\":\"agent-guardian\",\"version\":\"1.0.0\",\"event\":\"PolicyUpdated\",\"data\":{{\"max_per_tx\":\"{}\",\"daily_limit\":\"{}\"}}}}",
            self.max_per_tx, self.daily_limit
        ));
    }

    pub fn add_to_whitelist(&mut self, account_id: AccountId) {
        self.assert_owner();
        self.whitelist.insert(account_id.clone());
        env::log_str(&format!(
            "EVENT_JSON:{{\"standard\":\"agent-guardian\",\"version\":\"1.0.0\",\"event\":\"WhitelistAdd\",\"data\":{{\"account\":\"{account_id}\"}}}}",
        ));
    }

    pub fn remove_from_whitelist(&mut self, account_id: AccountId) {
        self.assert_owner();
        self.whitelist.remove(&account_id);
        env::log_str(&format!(
            "EVENT_JSON:{{\"standard\":\"agent-guardian\",\"version\":\"1.0.0\",\"event\":\"WhitelistRemove\",\"data\":{{\"account\":\"{account_id}\"}}}}",
        ));
    }

    pub fn pause(&mut self) {
        self.assert_owner();
        self.paused = true;
        env::log_str(
            "EVENT_JSON:{\"standard\":\"agent-guardian\",\"version\":\"1.0.0\",\"event\":\"Paused\",\"data\":{}}",
        );
    }

    pub fn unpause(&mut self) {
        self.assert_owner();
        self.paused = false;
        env::log_str(
            "EVENT_JSON:{\"standard\":\"agent-guardian\",\"version\":\"1.0.0\",\"event\":\"Unpaused\",\"data\":{}}",
        );
    }

    /// Owner withdraws treasury NEAR (not subject to agent policy or pause).
    pub fn withdraw(&mut self, amount: U128) -> Promise {
        self.assert_owner();
        require!(amount.0 > 0, "amount must be > 0");
        require!(
            spendable_yocto() >= amount.0,
            "insufficient spendable balance"
        );
        env::log_str(&format!(
            "EVENT_JSON:{{\"standard\":\"agent-guardian\",\"version\":\"1.0.0\",\"event\":\"Withdraw\",\"data\":{{\"amount\":\"{}\",\"to\":\"{}\"}}}}",
            amount.0, self.owner
        ));
        Promise::new(self.owner.clone()).transfer(NearToken::from_yoctonear(amount.0))
    }

    // ─── Agent spend path ────────────────────────────────────────────────────

    /// Agent entrypoint. Scope Function Call keys to this method only.
    ///
    /// On allow: updates `spent_today` and transfers `amount` yoctoNEAR to `receiver`.
    /// On deny: logs `SpendRejected` with reason and does **not** transfer.
    pub fn spend(&mut self, receiver: AccountId, amount: U128, memo: Option<String>) {
        let amount = amount.0;
        match self.authorize_spend(&receiver, amount) {
            Ok(()) => {
                self.spent_today = self
                    .spent_today
                    .checked_add(amount)
                    .unwrap_or_else(|| env::panic_str("spent_today overflow"));
                let memo = memo.unwrap_or_default();
                env::log_str(&format!(
                    "EVENT_JSON:{{\"standard\":\"agent-guardian\",\"version\":\"1.0.0\",\"event\":\"SpendAllowed\",\"data\":{{\"receiver\":\"{}\",\"amount\":\"{}\",\"memo\":\"{}\",\"spent_today\":\"{}\",\"day_bucket\":{}}}}}",
                    receiver, amount, escape_memo(&memo), self.spent_today, self.day_bucket
                ));
                // Promise is registered on drop even if not returned.
                let _ = Promise::new(receiver).transfer(NearToken::from_yoctonear(amount));
            }
            Err(reason) => {
                env::log_str(&format!(
                    "EVENT_JSON:{{\"standard\":\"agent-guardian\",\"version\":\"1.0.0\",\"event\":\"SpendRejected\",\"data\":{{\"receiver\":\"{receiver}\",\"amount\":\"{amount}\",\"reason\":\"{reason}\"}}}}",
                ));
            }
        }
    }

    // ─── Views ───────────────────────────────────────────────────────────────

    pub fn get_policy(&self) -> PolicyView {
        PolicyView {
            owner: self.owner.clone(),
            max_per_tx: U128(self.max_per_tx),
            daily_limit: U128(self.daily_limit),
            paused: self.paused,
            whitelist: self.whitelist.iter().cloned().collect(),
        }
    }

    pub fn get_spent_today(&self) -> U128 {
        let mut spent = self.spent_today;
        if current_day_bucket() != self.day_bucket {
            spent = 0;
        }
        U128(spent)
    }

    /// Effective UTC day. The stored bucket still advances lazily on the next
    /// `spend` or `set_policy`.
    pub fn get_day_bucket(&self) -> u64 {
        current_day_bucket()
    }

    pub fn can_spend(&self, receiver: AccountId, amount: U128) -> CanSpendView {
        let (spent, bucket) = self.effective_spent_today();
        let remaining = self.daily_limit.saturating_sub(spent);
        match self.evaluate_spend(&receiver, amount.0, spent) {
            Ok(()) => CanSpendView {
                allowed: true,
                reason: "ok".to_string(),
                spent_today: U128(spent),
                remaining_daily: U128(remaining),
                day_bucket: bucket,
            },
            Err(reason) => CanSpendView {
                allowed: false,
                reason,
                spent_today: U128(spent),
                remaining_daily: U128(remaining),
                day_bucket: bucket,
            },
        }
    }

    pub fn get_owner(&self) -> AccountId {
        self.owner.clone()
    }
}

impl Contract {
    fn assert_owner(&self) {
        require!(env::predecessor_account_id() == self.owner, "only owner");
    }

    fn roll_day_if_needed(&mut self) {
        let today = current_day_bucket();
        if today != self.day_bucket {
            self.day_bucket = today;
            self.spent_today = 0;
        }
    }

    fn effective_spent_today(&self) -> (u128, u64) {
        let today = current_day_bucket();
        if today != self.day_bucket {
            (0, today)
        } else {
            (self.spent_today, self.day_bucket)
        }
    }

    fn authorize_spend(&mut self, receiver: &AccountId, amount: u128) -> Result<(), String> {
        self.roll_day_if_needed();
        self.evaluate_spend(receiver, amount, self.spent_today)
    }

    /// Pure policy checks (shared by `spend` and `can_spend`).
    fn evaluate_spend(
        &self,
        receiver: &AccountId,
        amount: u128,
        spent_today: u128,
    ) -> Result<(), String> {
        if self.paused {
            return Err("paused".to_string());
        }
        if amount == 0 {
            return Err("amount_must_be_positive".to_string());
        }
        if !self.whitelist.contains(receiver) {
            return Err("not_whitelisted".to_string());
        }
        if amount > self.max_per_tx {
            return Err("over_max_per_tx".to_string());
        }
        let new_spent = spent_today
            .checked_add(amount)
            .ok_or_else(|| "spent_overflow".to_string())?;
        if new_spent > self.daily_limit {
            return Err("over_daily_limit".to_string());
        }
        if spendable_yocto() < amount {
            return Err("insufficient_treasury".to_string());
        }
        Ok(())
    }
}

/// NEAR that can leave this account.
///
/// `account_balance` includes the storage-staking reserve
/// (`storage_usage * storage_byte_cost`) and excludes validator
/// `account_locked_balance`. A transfer into the reserve fails the receipt,
/// so both withdrawal paths use this figure.
fn spendable_yocto() -> u128 {
    let balance = env::account_balance().as_yoctonear();
    let reserve =
        (env::storage_usage() as u128).saturating_mul(env::storage_byte_cost().as_yoctonear());
    balance.saturating_sub(reserve)
}

fn current_day_bucket() -> u64 {
    env::block_timestamp() / NS_PER_DAY
}

fn escape_memo(memo: &str) -> String {
    let mut out = String::with_capacity(memo.len());
    for ch in memo.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let code = c as u32;
                out.push_str(&format!("\\u{code:04x}"));
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::mock::MockAction;
    use near_sdk::test_utils::{accounts, get_created_receipts, get_logs, VMContextBuilder};
    use near_sdk::{testing_env, AccountId, NearToken};

    fn to_yocto(near: u128) -> u128 {
        near * 10u128.pow(24)
    }

    fn context(
        predecessor: AccountId,
        balance: NearToken,
        timestamp_ns: u64,
        is_view: bool,
    ) -> near_sdk::VMContext {
        VMContextBuilder::new()
            .current_account_id(accounts(0))
            .signer_account_id(predecessor.clone())
            .predecessor_account_id(predecessor)
            .account_balance(balance)
            .block_timestamp(timestamp_ns)
            .is_view(is_view)
            .build()
    }

    fn setup() -> Contract {
        let owner = accounts(1);
        testing_env!(context(
            owner.clone(),
            NearToken::from_near(100),
            NS_PER_DAY * 10,
            false
        ));
        let mut c = Contract::new(owner, U128(to_yocto(1)), U128(to_yocto(5)));
        c.add_to_whitelist(accounts(2)); // vendor
        c
    }

    #[test]
    fn allow_spend_when_under_limits_and_whitelisted() {
        let mut c = setup();
        let agent = accounts(3);
        testing_env!(context(
            agent,
            NearToken::from_near(100),
            NS_PER_DAY * 10,
            false
        ));
        c.spend(accounts(2), U128(to_yocto(1)), Some("ok".into()));
        assert_eq!(c.spent_today, to_yocto(1));
        let logs = get_logs().join("\n");
        assert!(logs.contains("SpendAllowed"), "logs: {logs}");
    }

    #[test]
    fn reject_over_max_per_tx() {
        let mut c = setup();
        testing_env!(context(
            accounts(3),
            NearToken::from_near(100),
            NS_PER_DAY * 10,
            false
        ));
        c.spend(accounts(2), U128(to_yocto(2)), None);
        assert_eq!(c.spent_today, 0);
        let logs = get_logs().join("\n");
        assert!(logs.contains("SpendRejected"), "logs: {logs}");
        assert!(logs.contains("over_max_per_tx"), "logs: {logs}");
    }

    #[test]
    fn reject_not_whitelisted() {
        let mut c = setup();
        testing_env!(context(
            accounts(3),
            NearToken::from_near(100),
            NS_PER_DAY * 10,
            false
        ));
        c.spend(accounts(4), U128(to_yocto(1)), None); // scam
        assert_eq!(c.spent_today, 0);
        let logs = get_logs().join("\n");
        assert!(logs.contains("not_whitelisted"), "logs: {logs}");
    }

    #[test]
    fn reject_when_would_exceed_daily_limit() {
        let mut c = setup();
        // max_per_tx = 1N, daily = 5N — five 1N spends OK; sixth fails.
        // Raise max so we can attempt 3N then 3N.
        testing_env!(context(
            accounts(1),
            NearToken::from_near(100),
            NS_PER_DAY * 10,
            false
        ));
        c.set_policy(U128(to_yocto(3)), U128(to_yocto(5)));

        testing_env!(context(
            accounts(3),
            NearToken::from_near(100),
            NS_PER_DAY * 10,
            false
        ));
        c.spend(accounts(2), U128(to_yocto(3)), None);
        assert_eq!(c.spent_today, to_yocto(3));
        c.spend(accounts(2), U128(to_yocto(3)), None);
        assert_eq!(c.spent_today, to_yocto(3));
        let logs = get_logs().join("\n");
        assert!(logs.contains("over_daily_limit"), "logs: {logs}");
    }

    #[test]
    fn reject_when_paused() {
        let mut c = setup();
        testing_env!(context(
            accounts(1),
            NearToken::from_near(100),
            NS_PER_DAY * 10,
            false
        ));
        c.pause();
        testing_env!(context(
            accounts(3),
            NearToken::from_near(100),
            NS_PER_DAY * 10,
            false
        ));
        c.spend(accounts(2), U128(to_yocto(1)), None);
        assert_eq!(c.spent_today, 0);
        let logs = get_logs().join("\n");
        assert!(logs.contains("\"reason\":\"paused\""), "logs: {logs}");
    }

    #[test]
    fn daily_bucket_resets_spent_today() {
        let mut c = setup();
        testing_env!(context(
            accounts(3),
            NearToken::from_near(100),
            NS_PER_DAY * 10,
            false
        ));
        c.spend(accounts(2), U128(to_yocto(1)), None);
        assert_eq!(c.spent_today, to_yocto(1));
        assert_eq!(c.day_bucket, 10);

        // Next UTC day → spend should reset counter then allow again.
        testing_env!(context(
            accounts(3),
            NearToken::from_near(100),
            NS_PER_DAY * 11,
            false
        ));
        c.spend(accounts(2), U128(to_yocto(1)), None);
        assert_eq!(c.day_bucket, 11);
        assert_eq!(c.spent_today, to_yocto(1));
        let view = c.can_spend(accounts(2), U128(to_yocto(1)));
        assert!(view.allowed);
        assert_eq!(view.spent_today.0, to_yocto(1));
    }

    #[test]
    fn can_spend_view_matches_policy() {
        let mut c = setup();
        // store::IterableSet flushes on Drop; flush before switching to view context.
        c.whitelist.flush();
        testing_env!(context(
            accounts(3),
            NearToken::from_near(100),
            NS_PER_DAY * 10,
            true
        ));
        let ok = c.can_spend(accounts(2), U128(to_yocto(1)));
        assert!(ok.allowed);
        assert_eq!(ok.reason, "ok");

        let bad = c.can_spend(accounts(4), U128(to_yocto(1)));
        assert!(!bad.allowed);
        assert_eq!(bad.reason, "not_whitelisted");
        // Keep contract alive past view asserts without Drop flush in view mode:
        // re-enter non-view before drop.
        testing_env!(context(
            accounts(1),
            NearToken::from_near(100),
            NS_PER_DAY * 10,
            false
        ));
        let _ = c.get_owner();
    }

    #[test]
    fn only_owner_can_set_policy() {
        let mut c = setup();
        testing_env!(context(
            accounts(3),
            NearToken::from_near(100),
            NS_PER_DAY * 10,
            false
        ));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            c.set_policy(U128(to_yocto(1)), U128(to_yocto(2)));
        }));
        assert!(result.is_err());
    }

    fn as_account(predecessor: AccountId, timestamp_ns: u64) {
        testing_env!(context(
            predecessor,
            NearToken::from_near(100),
            timestamp_ns,
            false
        ));
    }

    fn transfer_amounts() -> Vec<(String, u128)> {
        get_created_receipts()
            .into_iter()
            .flat_map(|receipt| {
                let receiver = receipt.receiver_id.to_string();
                receipt
                    .actions
                    .into_iter()
                    .filter_map(move |action| match action {
                        MockAction::Transfer { deposit, .. } => {
                            Some((receiver.clone(), deposit.as_yoctonear()))
                        }
                        _ => None,
                    })
            })
            .collect()
    }

    #[test]
    fn spend_transfers_exact_yocto_and_reject_does_not() {
        let mut c = setup();
        as_account(accounts(3), NS_PER_DAY * 10);
        c.spend(accounts(2), U128(1), Some("one-yocto".into()));
        assert_eq!(c.spent_today, 1);
        assert_eq!(transfer_amounts(), vec![(accounts(2).to_string(), 1)]);

        c.spend(accounts(4), U128(1), None);
        assert_eq!(c.spent_today, 1);
        assert_eq!(
            transfer_amounts().len(),
            1,
            "rejected spend adds no transfer"
        );
        let logs = get_logs().join("\n");
        assert!(logs.contains("not_whitelisted"), "{logs}");
    }

    #[test]
    fn zero_amount_is_rejected_without_transfer() {
        let mut c = setup();
        as_account(accounts(3), NS_PER_DAY * 10);
        c.spend(accounts(2), U128(0), None);
        assert_eq!(c.spent_today, 0);
        assert!(transfer_amounts().is_empty());
        let logs = get_logs().join("\n");
        assert!(logs.contains("amount_must_be_positive"), "{logs}");
    }

    #[test]
    fn pause_is_checked_before_amount_and_does_not_block_owner_withdraw() {
        let mut c = setup();
        as_account(accounts(1), NS_PER_DAY * 10);
        c.pause();
        as_account(accounts(3), NS_PER_DAY * 10);
        c.spend(accounts(2), U128(0), None);
        assert!(get_logs().join("\n").contains("\"reason\":\"paused\""));
        assert!(transfer_amounts().is_empty());

        as_account(accounts(1), NS_PER_DAY * 10);
        c.unpause();
        as_account(accounts(3), NS_PER_DAY * 10);
        c.spend(accounts(2), U128(to_yocto(1)), None);
        assert_eq!(c.spent_today, to_yocto(1));
    }

    #[test]
    fn owner_withdraw_pays_owner_and_is_outside_agent_caps() {
        let mut c = setup();
        as_account(accounts(1), NS_PER_DAY * 10);
        c.pause();
        let amount = to_yocto(2);
        let _ = c.withdraw(U128(amount));
        assert_eq!(
            c.spent_today, 0,
            "owner withdraw does not consume the daily cap"
        );
        assert_eq!(transfer_amounts(), vec![(accounts(1).to_string(), amount)]);
        let logs = get_logs().join("\n");
        assert!(logs.contains("Withdraw"), "{logs}");
    }

    #[test]
    #[should_panic(expected = "only owner")]
    fn non_owner_cannot_withdraw() {
        let mut c = setup();
        as_account(accounts(3), NS_PER_DAY * 10);
        let _ = c.withdraw(U128(1));
    }

    #[test]
    #[should_panic(expected = "only owner")]
    fn non_owner_cannot_pause() {
        let mut c = setup();
        as_account(accounts(3), NS_PER_DAY * 10);
        c.pause();
    }

    #[test]
    #[should_panic(expected = "only owner")]
    fn non_owner_cannot_unpause() {
        let mut c = setup();
        as_account(accounts(1), NS_PER_DAY * 10);
        c.pause();
        as_account(accounts(3), NS_PER_DAY * 10);
        c.unpause();
    }

    #[test]
    #[should_panic(expected = "only owner")]
    fn non_owner_cannot_add_to_whitelist() {
        let mut c = setup();
        as_account(accounts(3), NS_PER_DAY * 10);
        c.add_to_whitelist(accounts(4));
    }

    #[test]
    #[should_panic(expected = "only owner")]
    fn non_owner_cannot_remove_from_whitelist() {
        let mut c = setup();
        as_account(accounts(3), NS_PER_DAY * 10);
        c.remove_from_whitelist(accounts(2));
    }

    #[test]
    #[should_panic(expected = "amount must be > 0")]
    fn withdraw_rejects_zero() {
        let mut c = setup();
        as_account(accounts(1), NS_PER_DAY * 10);
        let _ = c.withdraw(U128(0));
    }

    #[test]
    #[should_panic(expected = "insufficient spendable balance")]
    fn withdraw_rejects_more_than_spendable_balance() {
        let mut c = setup();
        as_account(accounts(1), NS_PER_DAY * 10);
        let _ = c.withdraw(U128(to_yocto(100) + 1));
    }

    #[test]
    fn withdrawals_leave_storage_reserve_untouched() {
        let mut c = setup();
        let usage = env::storage_usage();
        let reserve = (usage as u128) * env::storage_byte_cost().as_yoctonear();
        let amount = to_yocto(1);
        let balance = reserve + amount - 1;
        testing_env!(VMContextBuilder::new()
            .current_account_id(accounts(0))
            .signer_account_id(accounts(3))
            .predecessor_account_id(accounts(3))
            .account_balance(NearToken::from_yoctonear(balance))
            .storage_usage(usage)
            .block_timestamp(NS_PER_DAY * 10)
            .build());
        let view = c.can_spend(accounts(2), U128(amount));
        assert!(!view.allowed);
        assert_eq!(view.reason, "insufficient_treasury");
        c.spend(accounts(2), U128(amount), None);
        assert_eq!(c.spent_today, 0);
        assert!(transfer_amounts().is_empty());

        testing_env!(VMContextBuilder::new()
            .current_account_id(accounts(0))
            .signer_account_id(accounts(1))
            .predecessor_account_id(accounts(1))
            .account_balance(NearToken::from_yoctonear(reserve + amount))
            .storage_usage(usage)
            .block_timestamp(NS_PER_DAY * 10)
            .build());
        c.spend(accounts(2), U128(amount), None);
        assert_eq!(c.spent_today, amount);
        assert_eq!(transfer_amounts(), vec![(accounts(2).to_string(), amount)]);
    }

    #[test]
    #[should_panic(expected = "insufficient spendable balance")]
    fn owner_cannot_withdraw_storage_reserve() {
        let mut c = setup();
        let usage = env::storage_usage();
        let reserve = (usage as u128) * env::storage_byte_cost().as_yoctonear();
        testing_env!(VMContextBuilder::new()
            .current_account_id(accounts(0))
            .signer_account_id(accounts(1))
            .predecessor_account_id(accounts(1))
            .account_balance(NearToken::from_yoctonear(reserve + 10))
            .storage_usage(usage)
            .block_timestamp(NS_PER_DAY * 10)
            .build());
        let _ = c.withdraw(U128(11));
    }

    #[test]
    fn exact_caps_allow_and_one_yocto_over_rejects() {
        let mut c = setup();
        as_account(accounts(3), NS_PER_DAY * 10);
        for i in 0..5 {
            c.spend(accounts(2), U128(to_yocto(1)), None);
            assert_eq!(c.spent_today, to_yocto(i + 1));
        }
        assert_eq!(transfer_amounts().len(), 5);
        c.spend(accounts(2), U128(1), None);
        assert_eq!(c.spent_today, to_yocto(5));
        assert!(get_logs().join("\n").contains("over_daily_limit"));
        assert_eq!(transfer_amounts().len(), 5);
    }

    #[test]
    fn views_apply_utc_bucket_before_lazy_state_rollover() {
        let mut c = setup();
        as_account(accounts(3), NS_PER_DAY * 10 + NS_PER_DAY - 1);
        c.spend(accounts(2), U128(to_yocto(1)), None);
        assert_eq!(c.day_bucket, 10);
        assert_eq!(current_day_bucket(), 10);

        c.whitelist.flush();
        testing_env!(context(
            accounts(3),
            NearToken::from_near(100),
            NS_PER_DAY * 11,
            true
        ));
        assert_eq!(current_day_bucket(), 11);
        assert_eq!(c.get_day_bucket(), 11);
        assert_eq!(c.get_spent_today().0, 0);
        assert_eq!(c.day_bucket, 10, "stored bucket waits for a mutation");
        assert_eq!(c.spent_today, to_yocto(1));
        let view = c.can_spend(accounts(2), U128(to_yocto(1)));
        assert!(view.allowed);
        assert_eq!(view.day_bucket, 11);
        assert_eq!(view.spent_today.0, 0);
        assert_eq!(view.remaining_daily.0, to_yocto(5));

        testing_env!(context(
            accounts(1),
            NearToken::from_near(100),
            NS_PER_DAY * 11,
            false
        ));
        c.set_policy(U128(to_yocto(1)), U128(to_yocto(5)));
        assert_eq!(c.day_bucket, 11);
        assert_eq!(c.spent_today, 0);
    }

    #[test]
    fn lowered_daily_limit_blocks_until_next_utc_bucket() {
        let mut c = setup();
        as_account(accounts(1), NS_PER_DAY * 10);
        c.set_policy(U128(to_yocto(3)), U128(to_yocto(5)));
        as_account(accounts(3), NS_PER_DAY * 10);
        c.spend(accounts(2), U128(to_yocto(3)), None);
        as_account(accounts(1), NS_PER_DAY * 10);
        c.set_policy(U128(to_yocto(1)), U128(to_yocto(2)));
        as_account(accounts(3), NS_PER_DAY * 10);
        c.spend(accounts(2), U128(to_yocto(1)), None);
        assert_eq!(c.spent_today, to_yocto(3));
        assert!(get_logs().join("\n").contains("over_daily_limit"));

        as_account(accounts(3), NS_PER_DAY * 11);
        c.spend(accounts(2), U128(to_yocto(1)), None);
        assert_eq!(c.day_bucket, 11);
        assert_eq!(c.spent_today, to_yocto(1));
    }

    #[test]
    fn remove_from_whitelist_blocks_that_receiver() {
        let mut c = setup();
        as_account(accounts(1), NS_PER_DAY * 10);
        c.remove_from_whitelist(accounts(2));
        c.remove_from_whitelist(accounts(4));
        as_account(accounts(3), NS_PER_DAY * 10);
        c.spend(accounts(2), U128(to_yocto(1)), None);
        assert_eq!(c.spent_today, 0);
        assert!(get_logs().join("\n").contains("not_whitelisted"));
    }

    #[test]
    fn any_predecessor_may_spend_when_policy_allows() {
        // Function Call key scope is an account-key fact. The contract applies
        // the same policy to every predecessor, including one that is not the owner.
        let mut c = setup();
        as_account(accounts(5), NS_PER_DAY * 10);
        c.spend(accounts(2), U128(to_yocto(1)), None);
        assert_eq!(c.spent_today, to_yocto(1));
        assert_eq!(
            transfer_amounts(),
            vec![(accounts(2).to_string(), to_yocto(1))]
        );
    }

    #[test]
    fn memo_event_stays_one_json_string() {
        let mut c = setup();
        as_account(accounts(3), NS_PER_DAY * 10);
        c.spend(
            accounts(2),
            U128(to_yocto(1)),
            Some("say \"hi\"\\\nnext".into()),
        );
        let event = get_logs()
            .into_iter()
            .find(|line| line.contains("SpendAllowed"))
            .expect("allowed event");
        assert!(!event.contains('\n'), "{event}");
        assert!(event.contains(r#"memo":"say \"hi\"\\\nnext""#), "{event}");
    }

    #[test]
    fn deposit_is_a_permissionless_credit() {
        let mut c = setup();
        testing_env!(VMContextBuilder::new()
            .current_account_id(accounts(0))
            .signer_account_id(accounts(4))
            .predecessor_account_id(accounts(4))
            .account_balance(NearToken::from_near(100))
            .attached_deposit(NearToken::from_near(1))
            .block_timestamp(NS_PER_DAY * 10)
            .build());
        c.deposit();
        let logs = get_logs().join("\n");
        assert!(logs.contains("Deposit"), "{logs}");
        assert!(logs.contains(&to_yocto(1).to_string()), "{logs}");
    }

    #[test]
    #[should_panic(expected = "attach NEAR to deposit")]
    fn deposit_rejects_zero_attached() {
        let mut c = setup();
        as_account(accounts(1), NS_PER_DAY * 10);
        c.deposit();
    }

    #[test]
    #[should_panic(expected = "max_per_tx must be > 0")]
    fn init_rejects_zero_max() {
        testing_env!(context(
            accounts(1),
            NearToken::from_near(100),
            NS_PER_DAY,
            false
        ));
        let _ = Contract::new(accounts(1), U128(0), U128(1));
    }

    #[test]
    #[should_panic(expected = "daily_limit must be >= max_per_tx")]
    fn init_rejects_daily_below_max() {
        testing_env!(context(
            accounts(1),
            NearToken::from_near(100),
            NS_PER_DAY,
            false
        ));
        let _ = Contract::new(accounts(1), U128(2), U128(1));
    }

    #[test]
    #[should_panic(expected = "daily_limit must be >= max_per_tx")]
    fn set_policy_rejects_daily_below_max() {
        let mut c = setup();
        as_account(accounts(1), NS_PER_DAY * 10);
        c.set_policy(U128(5), U128(4));
    }
}
