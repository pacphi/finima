//! Financial health scoring.
//!
//! Produces a composite 0-100 score from savings rate, debt ratio,
//! emergency fund months, and spending trend.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Inputs required to compute a financial health score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthScoreInput {
    pub monthly_income: Decimal,
    pub monthly_expenses: Decimal,
    pub total_assets: Decimal,
    pub total_liabilities: Decimal,
    pub liquid_savings: Decimal,
    pub avg_monthly_expenses: Decimal,
    /// Expenses in the most recent completed month.
    pub last_month_expenses: Decimal,
}

/// Computed financial health score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthScore {
    /// Composite score 0-100.
    pub score: u8,
    /// (income - expenses) / income.  0.0 when no income.
    pub savings_rate: f64,
    /// total_liabilities / total_assets.  0.0 when no assets.
    pub debt_ratio: f64,
    /// liquid_savings / avg_monthly_expenses.
    pub emergency_months: f64,
    /// -1 = spending decreasing, 0 = stable, 1 = increasing.
    pub spending_trend: i8,
}

// ---------------------------------------------------------------------------
// Computation
// ---------------------------------------------------------------------------

/// Compute financial health score from the provided inputs.
pub fn compute_health_score(data: &HealthScoreInput) -> HealthScore {
    let income_f = data.monthly_income.to_f64().unwrap_or(0.0);
    let expenses_f = data.monthly_expenses.to_f64().unwrap_or(0.0);
    let assets_f = data.total_assets.to_f64().unwrap_or(0.0);
    let liabilities_f = data.total_liabilities.to_f64().unwrap_or(0.0);
    let liquid_f = data.liquid_savings.to_f64().unwrap_or(0.0);
    let avg_expenses_f = data.avg_monthly_expenses.to_f64().unwrap_or(0.0);
    let last_month_f = data.last_month_expenses.to_f64().unwrap_or(0.0);

    // --- Component metrics ---

    let savings_rate = if income_f > 0.0 {
        ((income_f - expenses_f) / income_f).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let debt_ratio = if assets_f > 0.0 {
        (liabilities_f / assets_f).max(0.0)
    } else {
        0.0
    };

    let emergency_months = if avg_expenses_f > 0.0 {
        (liquid_f / avg_expenses_f).max(0.0)
    } else {
        0.0
    };

    // Spending trend: compare last month to 3-month average.
    let spending_trend: i8 = if avg_expenses_f > 0.0 {
        let ratio = last_month_f / avg_expenses_f;
        if ratio > 1.05 {
            1 // increasing
        } else if ratio < 0.95 {
            -1 // decreasing
        } else {
            0 // stable
        }
    } else {
        0
    };

    // --- Composite score (0-100) ---
    // Weights: savings_rate 30%, debt_ratio 25%, emergency_months 25%, spending_trend 20%.

    // savings_rate component: higher is better, 0..1 -> 0..100
    let savings_component = savings_rate * 100.0;

    // debt_ratio component: lower is better. 0 -> 100, >= 1.0 -> 0
    let debt_component = ((1.0 - debt_ratio).clamp(0.0, 1.0)) * 100.0;

    // emergency_months component: 6+ months -> 100, linear below
    let emergency_component = (emergency_months / 6.0).clamp(0.0, 1.0) * 100.0;

    // spending_trend component: decreasing -> 100, stable -> 75, increasing -> 25
    let trend_component = match spending_trend {
        -1 => 100.0,
        0 => 75.0,
        1 => 25.0,
        _ => 50.0,
    };

    let composite = savings_component * 0.30
        + debt_component * 0.25
        + emergency_component * 0.25
        + trend_component * 0.20;

    let score = composite.round().clamp(0.0, 100.0) as u8;

    HealthScore {
        score,
        savings_rate,
        debt_ratio,
        emergency_months,
        spending_trend,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn input(
        income: Decimal,
        expenses: Decimal,
        assets: Decimal,
        liabilities: Decimal,
        liquid: Decimal,
        avg_exp: Decimal,
        last_exp: Decimal,
    ) -> HealthScoreInput {
        HealthScoreInput {
            monthly_income: income,
            monthly_expenses: expenses,
            total_assets: assets,
            total_liabilities: liabilities,
            liquid_savings: liquid,
            avg_monthly_expenses: avg_exp,
            last_month_expenses: last_exp,
        }
    }

    #[test]
    fn zero_income_gives_zero_savings_rate() {
        let data = input(
            dec!(0),
            dec!(1000),
            dec!(10000),
            dec!(0),
            dec!(5000),
            dec!(1000),
            dec!(1000),
        );
        let result = compute_health_score(&data);
        assert_eq!(result.savings_rate, 0.0);
    }

    #[test]
    fn high_debt_ratio() {
        let data = input(
            dec!(5000),
            dec!(4000),
            dec!(10000),
            dec!(9000),
            dec!(3000),
            dec!(4000),
            dec!(4000),
        );
        let result = compute_health_score(&data);
        assert!(result.debt_ratio > 0.8);
    }

    #[test]
    fn perfect_score_scenario() {
        // High savings, no debt, 6+ months emergency, spending decreasing
        let data = input(
            dec!(10000),
            dec!(3000),
            dec!(100000),
            dec!(0),
            dec!(50000),
            dec!(3500),
            dec!(3000),
        );
        let result = compute_health_score(&data);
        assert!(
            result.score >= 85,
            "Expected high score, got {}",
            result.score
        );
        assert!(result.savings_rate > 0.5);
        assert_eq!(result.debt_ratio, 0.0);
        assert!(result.emergency_months > 6.0);
        assert_eq!(result.spending_trend, -1);
    }

    #[test]
    fn score_within_bounds() {
        let data = input(
            dec!(5000),
            dec!(4500),
            dec!(20000),
            dec!(15000),
            dec!(2000),
            dec!(4000),
            dec!(5000),
        );
        let result = compute_health_score(&data);
        assert!(result.score <= 100);
    }

    #[test]
    fn spending_trend_stable() {
        let data = input(
            dec!(5000),
            dec!(3000),
            dec!(50000),
            dec!(0),
            dec!(10000),
            dec!(3000),
            dec!(3000),
        );
        let result = compute_health_score(&data);
        assert_eq!(result.spending_trend, 0);
    }

    #[test]
    fn spending_trend_increasing() {
        let data = input(
            dec!(5000),
            dec!(3000),
            dec!(50000),
            dec!(0),
            dec!(10000),
            dec!(3000),
            dec!(4000),
        );
        let result = compute_health_score(&data);
        assert_eq!(result.spending_trend, 1);
    }

    #[test]
    fn no_assets_no_panic() {
        let data = input(
            dec!(3000),
            dec!(2500),
            dec!(0),
            dec!(5000),
            dec!(0),
            dec!(2500),
            dec!(2500),
        );
        let result = compute_health_score(&data);
        assert_eq!(result.debt_ratio, 0.0);
    }
}
