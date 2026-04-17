//! Shared date / month-range helpers.
//!
//! Consolidates the month-boundary arithmetic that previously lived
//! inline in multiple handlers and analysis routines. The key rule —
//! **month windows are half-open `[start, end_exclusive)`** — matches
//! how Monarch, Copilot, YNAB, Stripe, and the wider SQL / financial
//! reporting world bucket transactions. Using `<=` against a first-of-
//! next-month upper bound double-counts any row that lands on the 1st
//! (e.g. a mortgage posted Oct 1 leaking into September), which is the
//! bug that motivated this module.
//!
//! Callers should prefer [`month_range`] (returns both bounds) over
//! reaching for [`start_of_month`] / [`next_month_start`] individually;
//! the pair is harder to use incorrectly.

use chrono::{Datelike, NaiveDate};

use crate::Frequency;

/// Half-open `[start, end)` month window anchored on `anchor`'s month.
///
/// * `start` — first day of `anchor`'s month (e.g. `2025-09-01`)
/// * `end` — first day of the next month (e.g. `2025-10-01`), **exclusive**
///
/// Intended for SQL predicates of the form
/// `date >= start AND date < end`.
pub fn month_range(anchor: NaiveDate) -> (NaiveDate, NaiveDate) {
    let start = start_of_month(anchor);
    (start, next_month_start(start))
}

/// First day of the month containing `anchor`.
pub fn start_of_month(anchor: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(anchor.year(), anchor.month(), 1)
        .expect("every valid NaiveDate has a first-of-month")
}

/// First day of the month immediately after `anchor`'s month.
///
/// Handles the December → January rollover.
pub fn next_month_start(anchor: NaiveDate) -> NaiveDate {
    if anchor.month() == 12 {
        NaiveDate::from_ymd_opt(anchor.year() + 1, 1, 1)
            .expect("Jan 1 of next year is always valid")
    } else {
        NaiveDate::from_ymd_opt(anchor.year(), anchor.month() + 1, 1)
            .expect("month+1 ≤ 12 is always valid")
    }
}

/// Snap a posting date to its **billing cycle month** given the
/// recurring group's cadence and expected next-cycle anchor.
///
/// Anchors on the expected cycle date and finds the nearest cycle
/// (prior, current, or next) within half-a-cadence tolerance. If the
/// posting date is clearly outside any adjacent cycle the posting
/// month is returned unchanged — callers should treat that as
/// "posting-date attribution, nothing to snap."
///
/// This mirrors Copilot's "bills" behavior: a mortgage *due* Nov 1 that
/// *posts* Oct 31 is shown as **November's** mortgage on the Recurring
/// surface, because its billing cycle — not its posting date — is the
/// question the user is actually asking.
///
/// Returns the first-of-month of the attributed cycle (so callers can
/// `==`-compare with [`start_of_month`]).
///
/// `expected_cycle_anchor` is typically `recurring_groups.next_expected_date`
/// (the detector's projection) or any observed expected-cycle date.
/// For cadences without a well-defined fixed interval (Variable) the
/// posting month is returned as-is.
pub fn billing_cycle_month(
    posting_date: NaiveDate,
    cadence: Frequency,
    expected_cycle_anchor: NaiveDate,
) -> NaiveDate {
    let Some(interval_days) = nominal_interval_days(cadence) else {
        return start_of_month(posting_date);
    };
    // Half-cadence tolerance: a posting within ±interval/2 of an
    // expected cycle date is attributed to that cycle.
    let tolerance = (interval_days / 2).max(1);

    // Walk expected cycles forward/backward from the anchor. For
    // sub-month cadences (Daily/Weekly/Biweekly) we step by fixed
    // days; for monthly-and-longer cadences we step by calendar
    // months so a "monthly on the 1st" mortgage snaps to Oct 1 →
    // Nov 1 → Dec 1, not to Sep 30 → Oct 30 → Nov 29.
    let mut cycle = expected_cycle_anchor;
    let forward = posting_date >= cycle;

    let step = |d: NaiveDate, forward: bool| -> NaiveDate {
        match cadence {
            Frequency::Daily | Frequency::Weekly | Frequency::Biweekly => {
                if forward {
                    d + chrono::Duration::days(interval_days)
                } else {
                    d - chrono::Duration::days(interval_days)
                }
            }
            Frequency::Monthly
            | Frequency::Quarterly
            | Frequency::Semiannual
            | Frequency::Annual => {
                let months = match cadence {
                    Frequency::Monthly => 1,
                    Frequency::Quarterly => 3,
                    Frequency::Semiannual => 6,
                    Frequency::Annual => 12,
                    _ => unreachable!(),
                };
                if forward {
                    d.checked_add_months(chrono::Months::new(months))
                        .unwrap_or(d)
                } else {
                    d.checked_sub_months(chrono::Months::new(months))
                        .unwrap_or(d)
                }
            }
            Frequency::Variable => d,
        }
    };

    // Bounded walk — covers a few years in either direction for
    // pathological inputs while typically terminating in 0–2 steps.
    for _ in 0..256 {
        let diff = (posting_date - cycle).num_days().abs();
        if diff <= tolerance {
            return start_of_month(cycle);
        }
        let next = step(cycle, forward);
        if next == cycle {
            break;
        }
        if forward && next > posting_date + chrono::Duration::days(tolerance) {
            break;
        }
        if !forward && next < posting_date - chrono::Duration::days(tolerance) {
            break;
        }
        cycle = next;
    }

    // No cycle matched within tolerance — fall back to posting month.
    start_of_month(posting_date)
}

/// Nominal cadence length in days. Mirrors the private table in
/// [`crate::services`] / `finima-analysis::recurring`; kept here so
/// billing-cycle math doesn't need to depend on the analysis crate.
fn nominal_interval_days(freq: Frequency) -> Option<i64> {
    match freq {
        Frequency::Daily => Some(1),
        Frequency::Weekly => Some(7),
        Frequency::Biweekly => Some(14),
        Frequency::Monthly => Some(30),
        Frequency::Quarterly => Some(90),
        Frequency::Semiannual => Some(182),
        Frequency::Annual => Some(365),
        Frequency::Variable => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn month_range_september_is_half_open() {
        let (start, end) = month_range(d("2025-09-15"));
        assert_eq!(start, d("2025-09-01"));
        assert_eq!(end, d("2025-10-01"));
    }

    #[test]
    fn month_range_december_rolls_over() {
        let (start, end) = month_range(d("2025-12-31"));
        assert_eq!(start, d("2025-12-01"));
        assert_eq!(end, d("2026-01-01"));
    }

    #[test]
    fn next_month_start_december() {
        assert_eq!(next_month_start(d("2025-12-05")), d("2026-01-01"));
    }

    #[test]
    fn next_month_start_june() {
        assert_eq!(next_month_start(d("2025-06-05")), d("2025-07-01"));
    }

    #[test]
    fn billing_cycle_posting_day_before_due_attributes_to_due_month() {
        // Mortgage due Nov 1, posted Oct 31 → November's mortgage.
        let attributed = billing_cycle_month(d("2025-10-31"), Frequency::Monthly, d("2025-11-01"));
        assert_eq!(attributed, d("2025-11-01"));
    }

    #[test]
    fn billing_cycle_posting_on_due_date() {
        let attributed = billing_cycle_month(d("2025-11-01"), Frequency::Monthly, d("2025-11-01"));
        assert_eq!(attributed, d("2025-11-01"));
    }

    #[test]
    fn billing_cycle_walks_forward_multiple_cycles() {
        // Anchor is Aug 1, posting is Nov 2 — should land on November cycle.
        let attributed = billing_cycle_month(d("2025-11-02"), Frequency::Monthly, d("2025-08-01"));
        assert_eq!(attributed, d("2025-11-01"));
    }

    #[test]
    fn billing_cycle_walks_backward_multiple_cycles() {
        // Anchor is Nov 1, posting is Aug 2 — should land on August cycle.
        let attributed = billing_cycle_month(d("2025-08-02"), Frequency::Monthly, d("2025-11-01"));
        assert_eq!(attributed, d("2025-08-01"));
    }

    #[test]
    fn billing_cycle_out_of_tolerance_falls_back_to_posting_month() {
        // Posting is mid-month, no expected cycle within ±15 days → fall
        // back to posting's calendar month.
        let attributed = billing_cycle_month(d("2025-09-15"), Frequency::Monthly, d("2025-11-01"));
        assert_eq!(attributed, d("2025-09-01"));
    }

    #[test]
    fn billing_cycle_variable_returns_posting_month() {
        let attributed = billing_cycle_month(d("2025-09-15"), Frequency::Variable, d("2025-11-01"));
        assert_eq!(attributed, d("2025-09-01"));
    }

    #[test]
    fn billing_cycle_quarterly_near_anchor() {
        // Quarterly insurance due Jan 1, posted Dec 28 → attributes to January.
        let attributed =
            billing_cycle_month(d("2025-12-28"), Frequency::Quarterly, d("2026-01-01"));
        assert_eq!(attributed, d("2026-01-01"));
    }
}
