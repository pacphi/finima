# ADR-019: Align Recurring-Transaction Detection with Plaid Cadence Model

**Status:** Accepted
**Date:** 2026-04-17
**Related:** ADR-009 (Externalized YAML Configuration), ADR-012 (Tiered Categorization Engine)

## Context

The recurring-transaction detector in `crates/finima-analysis/src/recurring.rs`
classified a merchant as **Daily** whenever the median inter-transaction
interval was ≤ 1.5 days, and promoted a group to any fixed cadence
(Weekly/Biweekly/Monthly/Quarterly/Semiannual/Annual) with as few as **two**
matching transactions. Both rules produced false positives on the Recurring
page.

Observed symptoms (April 2026, "The Phillipsons" household data):

- **Amazon → Daily, $49.64, next 04/13/2026.** Amazon order bursts — two
  orders on the same day plus a third within 24h — pull the median interval
  to 1 day and trigger the Daily band even though the merchant is not a
  daily subscription.
- **Amsterdam Music Dome → Daily, $7.71.** Same pattern on a two-day burst.
- Other `two_transactions_monthly_gap`-style rows were being surfaced with
  only two data points, insufficient to distinguish real recurrence from
  coincidence.

### Industry practice

Research into how competitive and reference implementations handle this:

- **Plaid** (industry reference, used under the hood by Monarch, Copilot,
  Rocket Money, etc.) exposes `WEEKLY`, `BIWEEKLY`, `SEMI_MONTHLY`,
  `MONTHLY`, `ANNUALLY`, `UNKNOWN`. **There is no Daily cadence in the
  public enum.** A stream is only "matured" at **≥ 3 occurrences**; streams
  below that are held back under an `early_detection` status. Grouping uses
  description + amount + cadence jointly.
  - https://plaid.com/blog/recurring-transactions/
  - https://plaid.com/docs/api/products/transactions/

- **Monarch Money** routes detected candidates through a human
  _Recurring Review_ flow before they are treated as recurring; the detector
  can be loose because the user ratifies.
  - https://help.monarch.com/hc/en-us/articles/4890751141908-Tracking-Recurring-Expenses-and-Bills

- **Copilot Money** uses filter-based matching (name fragment + date range +
  amount range). Each recurring is a user-curated entity with a target
  cadence; no algorithmic auto-promotion.
  - https://help.copilot.money/en/articles/3783499-optimizing-recurrings
  - https://help.copilot.money/en/articles/3760068-creating-recurrings

- **Rocket Money** focuses on weekly/monthly/annual cadences and leans on
  merchant-catalog priors (e.g. "Netflix → monthly") rather than pure
  interval math.
  - https://help.rocketmoney.com/en/articles/2185531-managing-your-bills-and-subscriptions

- **Subaio** (European bank-embedded recurring-payment engine) describes a
  similarity-measure approach combining vendor + amount + temporal distance
  and a minimum-cluster-size gate — consistent with DBSCAN's `minPoints ≥ 3`
  convention.
  - https://subaio.com/subaio-explained/how-does-subaio-detect-recurring-payments

- **YNAB** does not auto-detect; users schedule recurring entries manually.

The common thread across non-manual detectors is: **no Daily band, and a
≥ 3 occurrence floor for fixed cadences.** Two-point promotions and
same-day-cluster "daily" classifications are industry anti-patterns.

## Decision

Align Finima's recurring detector with the Plaid cadence model:

1. **Drop the Daily classification band.** Patterns whose median interval is
   ≤ 1 day fall through to `Variable`, where the existing
   `min_occurrences_for_variable` / `variable_window_months` sliding-window
   gate already filters out bursty one-offs. The `Frequency::Daily` enum
   variant is **retained** so historical rows stored in the DB still
   deserialize; it is simply never emitted by the detector.
2. **Raise the minimum-occurrence floor for fixed sub-annual cadences to 3.**
   Weekly, Biweekly, Monthly, and Quarterly classifications now require at
   least 3 matching transactions (matching Plaid's "matured stream"
   threshold). Semiannual and Annual remain at 2 — requiring 3 samples
   would demand ≥ 2 years of contiguous history, which is unreasonable for
   this class of recurrence.
3. **Expose both thresholds via configuration.** The new floor is surfaced
   in `config/recurring.yaml` as `min_occurrences_for_fixed` (default 3) so
   operators can tune behavior without a code change, consistent with
   ADR-009.

## Consequences

### Positive

- Amazon-style burst patterns no longer register as "Daily" recurring
  charges; they either fall into `Variable` (and are gated by the existing
  window rule) or are dropped entirely.
- Two-point Weekly/Biweekly/Monthly/Quarterly promotions — which are
  statistically fragile — are suppressed until a third observation
  confirms the cadence.
- Finima's public cadence semantics match the industry reference (Plaid),
  making future integration (e.g. importing Plaid-enriched streams) and
  user expectations congruent.

### Negative / accepted tradeoffs

- The `two_transactions_monthly_gap` scenario (e.g. a brand-new Spotify
  subscription observed only twice) will not appear as Monthly until a
  third charge arrives. This delay is the intended cost of fewer false
  positives, and aligns with Plaid's `early_detection` semantics.
- A genuinely daily recurring charge (daily parking fee, certain FX/margin
  fees) will be classified as `Variable` rather than `Daily`. Given the
  rarity of such patterns in consumer finance, this is an acceptable cost;
  a user-driven "mark as daily" override can be added later if real
  demand emerges.
- The `Frequency::Daily` variant remains in the type, a minor piece of
  inert surface area retained for backwards compatibility with stored data.

### Neutral

- Annual-cost and next-expected-date computation for Semiannual/Annual
  streams continues to run on 2+ observations. These cadences are rare
  enough that demanding a 3rd sample would effectively disable them.

## Alternatives Considered

- **Add a density guard to the Daily band (≥ N occurrences in last 30
  days).** Rejected: introduces a third tuning knob for a pattern so rare
  that Plaid and peers don't bother supporting it at all. Simpler to drop
  the band.
- **Reject Daily only when amount coefficient-of-variation is high.**
  Rejected for now: requires deciding which merchant categories are
  legitimately variable-amount recurring (utilities, groceries) and which
  are not. Left as possible future work if dropping the band proves
  insufficient.
- **Leave the detector as-is and add a user confirmation flow.**
  Rejected: matches the Monarch/Copilot pattern but is substantially more
  product work. The detector changes here are a prerequisite even if a
  confirmation flow is added later.

## Implementation Notes

- `crates/finima-analysis/src/recurring.rs` — remove the Daily classification
  branch in `classify_frequency`; add `min_occurrences_for_fixed` to
  `RecurringDetectorConfig`; gate Weekly/Biweekly/Monthly/Quarterly on that
  threshold in `detect_recurring_with_config`.
- `config/recurring.yaml` — add `min_occurrences_for_fixed: 3`.
- `crates/finima-api/src/config.rs` — extend `RecurringConfig` with the
  new field and default.
- Unit tests updated to reflect the new semantics; a regression test added
  that simulates Amazon-style same-day bursts and asserts they are **not**
  classified as Daily.
- A maintainer CLI, `finima-redetect-recurring`, re-runs the detector
  across one or all portfolios so stored rows can be refreshed after
  classifier or threshold changes without waiting for the next file
  upload. See the
  [Maintainer Utilities Guide](../guides/maintainer-utilities.md#finima-redetect-recurring).
