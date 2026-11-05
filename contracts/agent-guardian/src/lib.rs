//! On-chain Agent Guardian — exact, public spending policy for AI agents on NEAR.
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
            "EVENT_JSON:{{\"standard\":\"agent-guardian\",\"version\":\"1.0.0\",\"event\":\"WhitelistAdd\",\"data\":{{\"account\":\"{}\"}}}}",
            account_id
        ));
    }

    pub fn remove_from_whitelist(&mut self, account_id: AccountId) {
        self.assert_owner();
        self.whitelist.remove(&account_id);
        env::log_str(&format!(
            "EVENT_JSON:{{\"standard\":\"agent-guardian\",\"version\":\"1.0.0\",\"event\":\"WhitelistRemove\",\"data\":{{\"account\":\"{}\"}}}}",
            account_id
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

    /// Owner withdraws treasury NEAR (not subject to agent policy).
    pub fn withdraw(&mut self, amount: U128) -> Promise {
        self.assert_owner();
        require!(amount.0 > 0, "amount must be > 0");
        let balance = env::account_balance().as_yoctonear();
        require!(balance >= amount.0, "insufficient contract balance");
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
                    "EVENT_JSON:{{\"standard\":\"agent-guardian\",\"version\":\"1.0.0\",\"event\":\"SpendRejected\",\"data\":{{\"receiver\":\"{}\",\"amount\":\"{}\",\"reason\":\"{}\"}}}}",
                    receiver, amount, reason
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

    pub fn get_day_bucket(&self) -> u64 {
        self.day_bucket
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
        require!(
            env::predecessor_account_id() == self.owner,
            "only owner"
        );
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
        let balance = env::account_balance().as_yoctonear();
        if balance < amount {
            return Err("insufficient_treasury".to_string());
        }
        Ok(())
    }
}

fn current_day_bucket() -> u64 {
    env::block_timestamp() / NS_PER_DAY
}

fn escape_memo(memo: &str) -> String {
    memo.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, get_logs, VMContextBuilder};
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
}
