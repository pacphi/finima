//! Resolves a transaction's canonical [`TransactionDirection`] from its
//! raw `amount` sign, given a configurable set of conventions.
//!
//! ## Why this exists
//!
//! Different financial institutions export the same business event with
//! opposite signs. American Express and Discover credit-card statements
//! show charges as positive amounts and payments as negative. Chase and
//! Citi (for some products) export the inverse. A presentation handler
//! cannot reliably infer "is this spending?" from amount sign without
//! knowing the source institution's convention.
//!
//! `SignNormalizer` centralizes this knowledge. It is consulted at
//! import time, not query time. Every persisted transaction carries
//! both a normalized [`TransactionDirection`] **and** a canonical
//! `amount` (positive_means_inflow) so that downstream consumers
//! (Sankey, reports, queries, balance computation) can trust the
//! sign of `amount` regardless of which institution exported the
//! row. See [`SignNormalizer::to_canonical_amount`] for the flip
//! rule and [`ADR-018`] for the design rationale.
//!
//! ## Resolution order
//!
//! When determining the convention for a transaction, [`SignNormalizer`]
//! consults rules in this order, returning the first match:
//!
//! 1. **Per-account override** (`by_account_id`) — set by the user via
//!    the UI ("Flip this account" on the Account detail page). Highest
//!    precedence.
//! 2. **Per-institution rule** (`by_institution`) — maintainer-curated
//!    in the YAML registry. Case-insensitive match on the account's
//!    institution name.
//! 3. **Account-type default** (`defaults_by_account_type`) — built-in
//!    sensible default. Asset accounts (checking/savings/cash) default
//!    to "positive means inflow"; liabilities (credit card, loan)
//!    default to "positive means outflow" (Amex/Discover convention).
//!
//! See ADR-018 (Import-Time Sign Normalization) for the full rationale.

use std::collections::HashMap;

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::types::{AccountType, TransactionDirection};

/// Which polarity a positive raw `amount` represents on a given account.
///
/// Persisted as a `TEXT` column on `accounts.sign_convention_override`
/// when set as a per-account user override. See ADR-018.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SignConvention {
    /// Positive `amount` = inflow (money in), negative = outflow.
    /// Standard convention for asset accounts (checking, savings) and
    /// for some institutions' credit-card exports (e.g. Chase).
    PositiveMeansInflow,
    /// Positive `amount` = outflow (money out), negative = inflow.
    /// Standard convention for most credit-card statements (Amex,
    /// Discover) and loans.
    PositiveMeansOutflow,
}

/// All resolution rules used by [`SignNormalizer`]. Built from the
/// project's YAML config + the user's per-account UI overrides.
#[derive(Debug, Clone, Default)]
pub struct SignConventions {
    /// User-set per-account overrides. Highest precedence.
    pub by_account_id: HashMap<Uuid, SignConvention>,
    /// Maintainer-curated per-institution rules. Keys are normalized
    /// to lowercase before storage; lookups also lowercase.
    pub by_institution: HashMap<String, SignConvention>,
    /// Built-in defaults by account type. Populated by
    /// [`Self::with_builtin_defaults`].
    pub defaults_by_account_type: HashMap<AccountType, SignConvention>,
}

impl SignConventions {
    /// Sensible built-in defaults: assets follow "positive = inflow";
    /// credit cards and loans follow "positive = outflow"
    /// (Amex/Discover convention). Institutions that diverge (Chase,
    /// Citi) override via `by_institution` from the YAML registry.
    pub fn with_builtin_defaults() -> Self {
        let mut defaults = HashMap::new();
        // Assets — positive amounts represent money entering the account.
        defaults.insert(AccountType::Checking, SignConvention::PositiveMeansInflow);
        defaults.insert(AccountType::Savings, SignConvention::PositiveMeansInflow);
        defaults.insert(AccountType::Cash, SignConvention::PositiveMeansInflow);
        defaults.insert(AccountType::Crypto, SignConvention::PositiveMeansInflow);
        defaults.insert(
            AccountType::InvestmentBrokerage,
            SignConvention::PositiveMeansInflow,
        );
        defaults.insert(
            AccountType::InvestmentRetirement,
            SignConvention::PositiveMeansInflow,
        );
        // Liabilities — positive amounts represent debt going up
        // (charges, loan disbursements).
        defaults.insert(
            AccountType::CreditCard,
            SignConvention::PositiveMeansOutflow,
        );
        defaults.insert(
            AccountType::LoanMortgage,
            SignConvention::PositiveMeansOutflow,
        );
        defaults.insert(AccountType::LoanAuto, SignConvention::PositiveMeansOutflow);
        defaults.insert(
            AccountType::LoanStudent,
            SignConvention::PositiveMeansOutflow,
        );
        defaults.insert(
            AccountType::LoanPersonal,
            SignConvention::PositiveMeansOutflow,
        );
        // "Other" — treat as asset for additive default.
        defaults.insert(AccountType::Other, SignConvention::PositiveMeansInflow);

        Self {
            by_account_id: HashMap::new(),
            by_institution: HashMap::new(),
            defaults_by_account_type: defaults,
        }
    }

    /// Insert a per-institution rule. Institution name is lowercased
    /// for consistent lookup. Returns the previous value if any.
    pub fn set_institution(
        &mut self,
        institution: impl Into<String>,
        convention: SignConvention,
    ) -> Option<SignConvention> {
        self.by_institution
            .insert(institution.into().to_lowercase(), convention)
    }

    /// Insert a per-account override. Returns the previous value if any.
    pub fn set_account(
        &mut self,
        account_id: Uuid,
        convention: SignConvention,
    ) -> Option<SignConvention> {
        self.by_account_id.insert(account_id, convention)
    }
}

/// Inputs needed to resolve a convention for a single transaction.
#[derive(Debug, Clone)]
pub struct AccountContext {
    pub account_id: Uuid,
    pub account_type: AccountType,
    /// Free-text institution name (e.g. "Chase", "American Express").
    /// Matched case-insensitively against `by_institution` rules.
    pub institution: Option<String>,
}

/// Service that maps raw transaction amounts to canonical
/// [`TransactionDirection`] using configured [`SignConventions`].
pub struct SignNormalizer {
    pub rules: SignConventions,
}

impl SignNormalizer {
    /// Construct a normalizer, merging the caller's rules with built-in
    /// account-type defaults so every account type has a fallback.
    pub fn new(mut rules: SignConventions) -> Self {
        let builtins = SignConventions::with_builtin_defaults();
        for (k, v) in builtins.defaults_by_account_type {
            rules.defaults_by_account_type.entry(k).or_insert(v);
        }
        Self { rules }
    }

    /// Compute the direction for a single transaction.
    pub fn direction_for(&self, ctx: &AccountContext, amount: Decimal) -> TransactionDirection {
        let convention = self.resolve(ctx);
        Self::apply(convention, amount)
    }

    /// Variant that lets the caller inject an autodetected convention
    /// (used during import when no explicit rule matches the
    /// account's institution). The detection result slots in between
    /// per-institution rules and account-type defaults.
    ///
    /// Resolution order with detection:
    ///   1. per-account override
    ///   2. per-institution rule
    ///   3. detected convention (Some)
    ///   4. account-type default
    pub fn direction_for_with_detection(
        &self,
        ctx: &AccountContext,
        amount: Decimal,
        detected: Option<SignConvention>,
    ) -> TransactionDirection {
        let convention = self.resolve_with_detection(ctx, detected);
        Self::apply(convention, amount)
    }

    /// Compute both the direction and the canonical-convention amount
    /// for a single row. The canonical convention is
    /// `PositiveMeansInflow`: positive amounts are always money in,
    /// negative amounts are always money out, regardless of what the
    /// source institution's file convention was.
    ///
    /// The returned `amount` is what the import pipeline persists to
    /// `transactions.amount`.
    pub fn normalize(
        &self,
        ctx: &AccountContext,
        raw_amount: Decimal,
    ) -> (TransactionDirection, Decimal) {
        self.normalize_with_detection(ctx, raw_amount, None)
    }

    /// Variant of [`Self::normalize`] that accepts an autodetected
    /// convention for use when no per-account or per-institution rule
    /// matches. See [`Self::direction_for_with_detection`] for the
    /// resolution chain.
    pub fn normalize_with_detection(
        &self,
        ctx: &AccountContext,
        raw_amount: Decimal,
        detected: Option<SignConvention>,
    ) -> (TransactionDirection, Decimal) {
        let convention = self.resolve_with_detection(ctx, detected);
        let direction = Self::apply(convention, raw_amount);
        let canonical = Self::to_canonical_amount(convention, raw_amount);
        (direction, canonical)
    }

    /// Flip `raw_amount` into the canonical (`PositiveMeansInflow`)
    /// convention given the source convention.
    ///
    /// The canonical sign rule is:
    /// - inflow rows have `amount >= 0`
    /// - outflow rows have `amount <= 0`
    ///
    /// This is invoked at import time so persisted rows have a single
    /// interpretation of `amount` sign regardless of the institution
    /// that produced them. See ADR-018.
    pub fn to_canonical_amount(convention: SignConvention, raw_amount: Decimal) -> Decimal {
        match convention {
            // Already canonical.
            SignConvention::PositiveMeansInflow => raw_amount,
            // Source convention is flipped relative to canonical —
            // negate so positive == inflow post-normalization.
            SignConvention::PositiveMeansOutflow => -raw_amount,
        }
    }

    /// Resolve the effective [`SignConvention`] for an account, using
    /// the full rule chain (per-account override → per-institution
    /// rule → account-type default). Does not consult autodetection.
    ///
    /// This is the canonical way for callers to ask "what convention
    /// is currently in effect on this account?" — prefer it over
    /// probing via [`Self::direction_for`] with a fixed sign, which
    /// is more fragile and obscures intent.
    pub fn resolve_convention(&self, ctx: &AccountContext) -> SignConvention {
        self.resolve_with_detection(ctx, None)
    }

    /// Resolve the effective [`SignConvention`] including the
    /// autodetection slot. Identical ordering to
    /// [`Self::direction_for_with_detection`].
    pub fn resolve_convention_with_detection(
        &self,
        ctx: &AccountContext,
        detected: Option<SignConvention>,
    ) -> SignConvention {
        self.resolve_with_detection(ctx, detected)
    }

    fn resolve(&self, ctx: &AccountContext) -> SignConvention {
        self.resolve_with_detection(ctx, None)
    }

    fn resolve_with_detection(
        &self,
        ctx: &AccountContext,
        detected: Option<SignConvention>,
    ) -> SignConvention {
        if let Some(c) = self.rules.by_account_id.get(&ctx.account_id) {
            return *c;
        }
        if let Some(inst) = ctx.institution.as_ref() {
            if let Some(c) = self.rules.by_institution.get(&inst.to_lowercase()) {
                return *c;
            }
        }
        if let Some(c) = detected {
            return c;
        }
        self.rules
            .defaults_by_account_type
            .get(&ctx.account_type)
            .copied()
            .unwrap_or(SignConvention::PositiveMeansInflow)
    }

    fn apply(convention: SignConvention, amount: Decimal) -> TransactionDirection {
        // Decimal::ZERO is treated as inflow (rare; reversals/placeholders).
        let is_negative = amount.is_sign_negative() && !amount.is_zero();
        match (convention, is_negative) {
            (SignConvention::PositiveMeansInflow, false) => TransactionDirection::Inflow,
            (SignConvention::PositiveMeansInflow, true) => TransactionDirection::Outflow,
            (SignConvention::PositiveMeansOutflow, false) => TransactionDirection::Outflow,
            (SignConvention::PositiveMeansOutflow, true) => TransactionDirection::Inflow,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(account_type: AccountType, institution: Option<&str>) -> AccountContext {
        AccountContext {
            account_id: Uuid::nil(),
            account_type,
            institution: institution.map(String::from),
        }
    }

    fn dec(n: i64) -> Decimal {
        Decimal::new(n, 0)
    }

    // ── Built-in defaults ────────────────────────────────────────────

    #[test]
    fn checking_positive_is_inflow_by_default() {
        let n = SignNormalizer::new(SignConventions::default());
        let c = ctx(AccountType::Checking, None);
        assert_eq!(n.direction_for(&c, dec(100)), TransactionDirection::Inflow);
        assert_eq!(n.direction_for(&c, dec(-50)), TransactionDirection::Outflow);
    }

    #[test]
    fn credit_card_positive_is_outflow_by_default() {
        let n = SignNormalizer::new(SignConventions::default());
        let c = ctx(AccountType::CreditCard, Some("amex"));
        assert_eq!(
            n.direction_for(&c, dec(28)),
            TransactionDirection::Outflow,
            "Amex charge (positive) is outflow under default convention"
        );
        assert_eq!(
            n.direction_for(&c, dec(-240)),
            TransactionDirection::Inflow,
            "Amex payment (negative) is inflow under default convention"
        );
    }

    #[test]
    fn savings_uses_inflow_default() {
        let n = SignNormalizer::new(SignConventions::default());
        assert_eq!(
            n.direction_for(&ctx(AccountType::Savings, None), dec(500)),
            TransactionDirection::Inflow
        );
    }

    // ── Per-institution overrides ───────────────────────────────────

    #[test]
    fn chase_credit_card_is_inverted() {
        let mut rules = SignConventions::default();
        rules.set_institution("chase", SignConvention::PositiveMeansInflow);
        let n = SignNormalizer::new(rules);
        let c = ctx(AccountType::CreditCard, Some("Chase"));
        assert_eq!(
            n.direction_for(&c, dec(-53)),
            TransactionDirection::Outflow,
            "Chase charge (negative) is outflow"
        );
        assert_eq!(
            n.direction_for(&c, dec(1500)),
            TransactionDirection::Inflow,
            "Chase payment (positive) is inflow"
        );
    }

    #[test]
    fn institution_lookup_is_case_insensitive() {
        let mut rules = SignConventions::default();
        rules.set_institution("CHASE", SignConvention::PositiveMeansInflow);
        let n = SignNormalizer::new(rules);
        assert_eq!(
            n.direction_for(&ctx(AccountType::CreditCard, Some("chase")), dec(100)),
            TransactionDirection::Inflow
        );
        assert_eq!(
            n.direction_for(&ctx(AccountType::CreditCard, Some("Chase")), dec(100)),
            TransactionDirection::Inflow
        );
    }

    // ── Per-account overrides ───────────────────────────────────────

    #[test]
    fn account_override_beats_institution() {
        let acct = Uuid::new_v4();
        let mut rules = SignConventions::default();
        rules.set_institution("chase", SignConvention::PositiveMeansInflow);
        rules.set_account(acct, SignConvention::PositiveMeansOutflow);
        let n = SignNormalizer::new(rules);
        let c = AccountContext {
            account_id: acct,
            account_type: AccountType::CreditCard,
            institution: Some("chase".into()),
        };
        // Account override flips back to PositiveMeansOutflow.
        assert_eq!(n.direction_for(&c, dec(100)), TransactionDirection::Outflow);
    }

    #[test]
    fn account_override_beats_default() {
        let acct = Uuid::new_v4();
        let mut rules = SignConventions::default();
        rules.set_account(acct, SignConvention::PositiveMeansInflow);
        let n = SignNormalizer::new(rules);
        let c = AccountContext {
            account_id: acct,
            account_type: AccountType::CreditCard,
            institution: None,
        };
        // Default would say PositiveMeansOutflow; override flips it.
        assert_eq!(n.direction_for(&c, dec(100)), TransactionDirection::Inflow);
    }

    // ── Detection slot ──────────────────────────────────────────────

    #[test]
    fn detection_used_when_no_institution_rule() {
        let n = SignNormalizer::new(SignConventions::default());
        let c = ctx(AccountType::CreditCard, Some("unknown_bank"));
        assert_eq!(
            n.direction_for_with_detection(&c, dec(100), Some(SignConvention::PositiveMeansInflow)),
            TransactionDirection::Inflow,
            "detected convention overrides default for unknown institution"
        );
    }

    #[test]
    fn detection_loses_to_institution_rule() {
        let mut rules = SignConventions::default();
        rules.set_institution("amex", SignConvention::PositiveMeansOutflow);
        let n = SignNormalizer::new(rules);
        let c = ctx(AccountType::CreditCard, Some("amex"));
        assert_eq!(
            n.direction_for_with_detection(
                &c,
                dec(100),
                Some(SignConvention::PositiveMeansInflow),
            ),
            TransactionDirection::Outflow,
            "explicit institution rule wins over detection"
        );
    }

    #[test]
    fn detection_loses_to_account_override() {
        let acct = Uuid::new_v4();
        let mut rules = SignConventions::default();
        rules.set_account(acct, SignConvention::PositiveMeansOutflow);
        let n = SignNormalizer::new(rules);
        let c = AccountContext {
            account_id: acct,
            account_type: AccountType::CreditCard,
            institution: None,
        };
        assert_eq!(
            n.direction_for_with_detection(
                &c,
                dec(100),
                Some(SignConvention::PositiveMeansInflow),
            ),
            TransactionDirection::Outflow,
            "user override wins over detection"
        );
    }

    // ── Edge cases ──────────────────────────────────────────────────

    #[test]
    fn zero_is_inflow_under_either_convention() {
        let n = SignNormalizer::new(SignConventions::default());
        assert_eq!(
            n.direction_for(&ctx(AccountType::Checking, None), Decimal::ZERO),
            TransactionDirection::Inflow
        );
        assert_eq!(
            n.direction_for(&ctx(AccountType::CreditCard, Some("amex")), Decimal::ZERO),
            TransactionDirection::Outflow,
            "zero on a positive_means_outflow account becomes outflow (rare; reversals)"
        );
    }

    #[test]
    fn missing_institution_falls_back_to_default() {
        let mut rules = SignConventions::default();
        rules.set_institution("chase", SignConvention::PositiveMeansInflow);
        let n = SignNormalizer::new(rules);
        let c = ctx(AccountType::CreditCard, None); // no institution
        assert_eq!(
            n.direction_for(&c, dec(100)),
            TransactionDirection::Outflow,
            "no institution => use account-type default (PositiveMeansOutflow)"
        );
    }

    // ── Canonical amount normalization ──────────────────────────────

    #[test]
    fn canonical_amount_passthrough_for_inflow_convention() {
        // Chase-like source: positive_means_inflow. Raw amount is
        // already canonical, so no sign flip.
        let c = SignConvention::PositiveMeansInflow;
        assert_eq!(
            SignNormalizer::to_canonical_amount(c, dec(725)),
            dec(725),
            "positive inflow stays positive"
        );
        assert_eq!(
            SignNormalizer::to_canonical_amount(c, dec(-115)),
            dec(-115),
            "negative outflow stays negative"
        );
    }

    #[test]
    fn canonical_amount_flips_for_outflow_convention() {
        // Amex-like source: positive_means_outflow. Every amount is
        // flipped so positive becomes money-out becomes negative.
        let c = SignConvention::PositiveMeansOutflow;
        assert_eq!(
            SignNormalizer::to_canonical_amount(c, dec(28)),
            dec(-28),
            "Amex charge (raw +28) flips to canonical -28"
        );
        assert_eq!(
            SignNormalizer::to_canonical_amount(c, dec(-240)),
            dec(240),
            "Amex payment (raw -240) flips to canonical +240"
        );
    }

    #[test]
    fn normalize_returns_direction_and_canonical_amount_together() {
        let mut rules = SignConventions::default();
        rules.set_institution("chase", SignConvention::PositiveMeansInflow);
        let n = SignNormalizer::new(rules);

        // Chase card, raw payment +725.
        let (dir, amt) = n.normalize(&ctx(AccountType::CreditCard, Some("chase")), dec(725));
        assert_eq!(dir, TransactionDirection::Inflow);
        assert_eq!(amt, dec(725), "Chase payment canonical amount = +725");

        // Amex card (default convention), raw charge +28.
        let (dir, amt) = n.normalize(&ctx(AccountType::CreditCard, Some("amex")), dec(28));
        assert_eq!(dir, TransactionDirection::Outflow);
        assert_eq!(amt, dec(-28), "Amex charge canonical amount = -28");
    }

    #[test]
    fn normalize_invariant_sign_matches_direction() {
        // After canonicalization, non-zero amounts satisfy:
        //   inflow  <=> amount > 0
        //   outflow <=> amount < 0
        // Check across both source conventions and both account roles.
        let cases: &[(SignConvention, AccountType, &str, i64)] = &[
            (
                SignConvention::PositiveMeansOutflow,
                AccountType::CreditCard,
                "amex",
                28,
            ),
            (
                SignConvention::PositiveMeansOutflow,
                AccountType::CreditCard,
                "amex",
                -240,
            ),
            (
                SignConvention::PositiveMeansInflow,
                AccountType::CreditCard,
                "chase",
                725,
            ),
            (
                SignConvention::PositiveMeansInflow,
                AccountType::CreditCard,
                "chase",
                -115,
            ),
            (
                SignConvention::PositiveMeansInflow,
                AccountType::Checking,
                "any",
                5000,
            ),
            (
                SignConvention::PositiveMeansInflow,
                AccountType::Checking,
                "any",
                -85,
            ),
        ];
        for (conv, ty, inst, raw_units) in cases.iter().copied() {
            let mut rules = SignConventions::default();
            rules.set_institution(inst, conv);
            let n = SignNormalizer::new(rules);
            let (dir, amt) = n.normalize(&ctx(ty, Some(inst)), dec(raw_units));
            match dir {
                TransactionDirection::Inflow => assert!(
                    amt >= Decimal::ZERO,
                    "inflow should have non-negative canonical amount, got {amt} for {inst} {raw_units}"
                ),
                TransactionDirection::Outflow => assert!(
                    amt <= Decimal::ZERO,
                    "outflow should have non-positive canonical amount, got {amt} for {inst} {raw_units}"
                ),
            }
        }
    }
}
