//! Financial analysis module for Finima.
//!
//! Provides recurring payment detection, budget computation,
//! financial health scoring, cash flow analysis, and flow detection.

pub mod budget;
pub mod cashflow;
pub mod flows;
pub mod health_score;
pub mod net_worth;
pub mod recurring;
pub mod sona;

pub use budget::{
    auto_suggest_budgets, compute_budget_vs_actual, BudgetEntry, BudgetSuggestion, BudgetVsActual,
};
pub use cashflow::{compute_monthly_cashflow, MonthlyCashFlow};
pub use flows::{
    build_outflow_ranking, build_sankey_data, build_waterfall, detect_flows, FlowCandidate,
    FlowRecord, OutflowRank, SankeyData, SankeyLink, SankeyNode, WaterfallData, WaterfallSegment,
};
pub use health_score::{compute_health_score, HealthScore, HealthScoreInput};
pub use net_worth::{compute_net_worth_series, AccountSnapshot, NetWorthPoint};
pub use recurring::{
    detect_recurring, detect_recurring_with_config, RecurringDetector, RecurringDetectorConfig,
    RecurringGroupCandidate, TransactionForAnalysis,
};
