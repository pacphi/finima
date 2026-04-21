//! Maintainer utility that generates a committed sample dataset for demos.
//!
//! The output lives at `data/sample/sample.sql` and models a joint-household
//! portfolio (Chase Bank joint checking + savings, Amex Reserve, Atmos Visa,
//! Mazda CX-90 auto loan, Charles Schwab brokerage) over 18 months working
//! back from 2026-04-20.
//!
//! The generator is **self-contained** (no DB connection) and **fully
//! deterministic**: a fixed splitmix64 seed means re-running it produces a
//! byte-identical `sample.sql` (`git diff` is the verification).
//!
//! Amount strategy (see plan):
//!   - Recurring / fixed-cost items are pinned to round numbers.
//!   - Variable spend is sampled from clipped distributions (p5–p95).
//!
//! Load with `make sample-load`; purge with `make sample-purge`.
//!
//! Usage:
//!   cargo run -p finima-api --bin finima-generate-sample [-- --out PATH]

use std::fs;
use std::path::PathBuf;

use chrono::{Datelike, Duration, NaiveDate, Weekday};

// ---------------------------------------------------------------------------
// Deterministic RNG (splitmix64). Self-contained so re-runs are byte-stable
// regardless of upstream rand-crate algorithm changes.
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform f64 in `[0.0, 1.0)`.
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Uniform amount in `[lo, hi]` rounded to cents.
    fn amount(&mut self, lo: f64, hi: f64) -> f64 {
        let v = lo + self.unit() * (hi - lo);
        (v * 100.0).round() / 100.0
    }
    fn choose<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[(self.next_u64() as usize) % xs.len()]
    }
}

// ---------------------------------------------------------------------------
// Fixed identifiers (deterministic UUIDs — easy to reference in tests/demos)
// ---------------------------------------------------------------------------

const USER_ID: &str = "a2000000-0000-4000-8000-000000000001";
const PORTFOLIO_ID: &str = "b2000000-0000-4000-8000-000000000001";

// Account UUIDs.
const ACC_CHECKING: &str = "c2000000-0000-4000-8000-000000000001";
const ACC_SAVINGS: &str = "c2000000-0000-4000-8000-000000000002";
const ACC_AMEX: &str = "c2000000-0000-4000-8000-000000000003";
const ACC_ATMOS: &str = "c2000000-0000-4000-8000-000000000004";
const ACC_MAZDA: &str = "c2000000-0000-4000-8000-000000000005";
const ACC_SCHWAB: &str = "c2000000-0000-4000-8000-000000000006";

// Recurring group UUIDs.
const RG_PAYROLL: &str = "e2000000-0000-4000-8000-000000000001";
const RG_RENT: &str = "e2000000-0000-4000-8000-000000000002";
const RG_ELECTRIC: &str = "e2000000-0000-4000-8000-000000000003";
const RG_GAS_UTIL: &str = "e2000000-0000-4000-8000-000000000004";
const RG_INTERNET: &str = "e2000000-0000-4000-8000-000000000005";
const RG_WATER: &str = "e2000000-0000-4000-8000-000000000006";
const RG_NETFLIX: &str = "e2000000-0000-4000-8000-000000000007";
const RG_SPOTIFY: &str = "e2000000-0000-4000-8000-000000000008";
const RG_GYM: &str = "e2000000-0000-4000-8000-000000000009";
const RG_MAZDA_PMT: &str = "e2000000-0000-4000-8000-00000000000a";
const RG_INVEST_ACH: &str = "e2000000-0000-4000-8000-00000000000b";

const SAVINGS_GOAL_ID: &str = "f2000000-0000-4000-8000-000000000001";

// 18-month window working back from 2026-04-20.
const END_DATE: (i32, u32, u32) = (2026, 4, 20);
const START_DATE: (i32, u32, u32) = (2024, 10, 20);

// Seed fixed so regeneration is byte-stable.
const RNG_SEED: u64 = 0xF104_9A5A_1177_CC42;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Txn {
    id: String,
    account_id: &'static str,
    date: NaiveDate,
    description: &'static str,
    amount: f64,
    category: &'static str,
    subcategory: Option<&'static str>,
    merchant: &'static str,
    is_recurring: bool,
    recurring_group: Option<&'static str>,
}

impl Txn {
    fn direction(&self) -> &'static str {
        if self.amount >= 0.0 {
            "inflow"
        } else {
            "outflow"
        }
    }
}

struct Flow {
    id: String,
    source_account: &'static str,
    target_account: &'static str,
    source_txn: String,
    target_txn: String,
    amount: f64,
    flow_date: NaiveDate,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

fn txn_uuid(seq: u32) -> String {
    // Use a fixed prefix so all generated transactions share a namespace.
    format!("d2000000-0000-4000-8000-{:012x}", seq as u64)
}

fn flow_uuid(seq: u32) -> String {
    format!("f3000000-0000-4000-8000-{:012x}", seq as u64)
}

fn sql_str(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn sql_opt_str(s: Option<&str>) -> String {
    match s {
        Some(v) => sql_str(v),
        None => "NULL".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Main generation
// ---------------------------------------------------------------------------

fn main() {
    let out_path = std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|w| w[0] == "--out")
        .map(|w| PathBuf::from(&w[1]))
        .unwrap_or_else(|| PathBuf::from("data/sample/sample.sql"));

    let mut rng = Rng::new(RNG_SEED);

    let start = date(START_DATE.0, START_DATE.1, START_DATE.2);
    let end = date(END_DATE.0, END_DATE.1, END_DATE.2);

    let mut txns: Vec<Txn> = Vec::new();
    let mut flows: Vec<Flow> = Vec::new();
    let mut txn_seq: u32 = 1;
    let mut flow_seq: u32 = 1;

    // -----------------------------------------------------------------
    // Biweekly payroll (pinned $6,400) — every other Friday from start.
    // -----------------------------------------------------------------
    {
        let mut d = start;
        while d.weekday() != Weekday::Fri {
            d = d.succ_opt().unwrap();
        }
        let mut toggle = true;
        while d <= end {
            if toggle {
                let id = txn_uuid(txn_seq);
                txn_seq += 1;
                txns.push(Txn {
                    id,
                    account_id: ACC_CHECKING,
                    date: d,
                    description: "EMPLOYER DIRECT DEPOSIT",
                    amount: 6400.00,
                    category: "income",
                    subcategory: Some("salary"),
                    merchant: "Employer Payroll",
                    is_recurring: true,
                    recurring_group: Some(RG_PAYROLL),
                });
            }
            toggle = !toggle;
            d += Duration::days(7);
        }
    }

    // Helper: iterate first-of-month dates in window.
    let months: Vec<NaiveDate> = {
        let mut v = Vec::new();
        let mut y = start.year();
        let mut m = start.month();
        while date(y, m, 1) <= end {
            v.push(date(y, m, 1));
            m += 1;
            if m > 12 {
                m = 1;
                y += 1;
            }
        }
        v
    };

    // -----------------------------------------------------------------
    // Monthly fixed / recurring expenses on Joint Checking.
    // -----------------------------------------------------------------
    for mstart in &months {
        let y = mstart.year();
        let m = mstart.month();
        // Rent — 1st of month — $2,900.
        let id = txn_uuid(txn_seq);
        txn_seq += 1;
        txns.push(Txn {
            id,
            account_id: ACC_CHECKING,
            date: date(y, m, 1),
            description: "RENT PAYMENT ACH",
            amount: -2900.00,
            category: "housing",
            subcategory: Some("rent"),
            merchant: "Landlord ACH",
            is_recurring: true,
            recurring_group: Some(RG_RENT),
        });

        // Internet — 3rd — $89.99.
        let id = txn_uuid(txn_seq);
        txn_seq += 1;
        txns.push(Txn {
            id,
            account_id: ACC_CHECKING,
            date: date(y, m, 3),
            description: "COMCAST XFINITY",
            amount: -89.99,
            category: "utilities",
            subcategory: Some("internet"),
            merchant: "Comcast",
            is_recurring: true,
            recurring_group: Some(RG_INTERNET),
        });

        // Water — 7th — $45.00.
        let id = txn_uuid(txn_seq);
        txn_seq += 1;
        txns.push(Txn {
            id,
            account_id: ACC_CHECKING,
            date: date(y, m, 7),
            description: "CITY WATER UTILITY",
            amount: -45.00,
            category: "utilities",
            subcategory: Some("water_sewer"),
            merchant: "City Water",
            is_recurring: true,
            recurring_group: Some(RG_WATER),
        });

        // Electric — 15th — ~$120 ±5%.
        let id = txn_uuid(txn_seq);
        txn_seq += 1;
        let elec = rng.amount(114.0, 126.0);
        txns.push(Txn {
            id,
            account_id: ACC_CHECKING,
            date: date(y, m, 15),
            description: "PACIFIC POWER ELEC",
            amount: -elec,
            category: "utilities",
            subcategory: Some("electricity"),
            merchant: "Pacific Power",
            is_recurring: true,
            recurring_group: Some(RG_ELECTRIC),
        });

        // Natural gas — 15th — ~$45 ±10%.
        let id = txn_uuid(txn_seq);
        txn_seq += 1;
        let ngas = rng.amount(40.0, 50.0);
        txns.push(Txn {
            id,
            account_id: ACC_CHECKING,
            date: date(y, m, 15),
            description: "NW NATURAL GAS",
            amount: -ngas,
            category: "utilities",
            subcategory: Some("gas"),
            merchant: "NW Natural",
            is_recurring: true,
            recurring_group: Some(RG_GAS_UTIL),
        });

        // Mazda loan payment — 15th — $625 from checking; principal paydown
        // inflow on the loan account. Represent as two paired txns + flow.
        let src_id = txn_uuid(txn_seq);
        txn_seq += 1;
        let tgt_id = txn_uuid(txn_seq);
        txn_seq += 1;
        let pmt_date = date(y, m, 15);
        txns.push(Txn {
            id: src_id.clone(),
            account_id: ACC_CHECKING,
            date: pmt_date,
            description: "MAZDA FINANCIAL PAYMENT",
            amount: -625.00,
            category: "debt_payment",
            subcategory: Some("auto_loan"),
            merchant: "Mazda Financial",
            is_recurring: true,
            recurring_group: Some(RG_MAZDA_PMT),
        });
        txns.push(Txn {
            id: tgt_id.clone(),
            account_id: ACC_MAZDA,
            date: pmt_date,
            description: "PRINCIPAL PAYMENT",
            amount: 625.00,
            category: "transfer",
            subcategory: Some("loan_paydown"),
            merchant: "Mazda Financial",
            is_recurring: true,
            recurring_group: None,
        });
        flows.push(Flow {
            id: flow_uuid(flow_seq),
            source_account: ACC_CHECKING,
            target_account: ACC_MAZDA,
            source_txn: src_id,
            target_txn: tgt_id,
            amount: 625.00,
            flow_date: pmt_date,
        });
        flow_seq += 1;

        // Monthly investment ACH — 20th — $1,500 checking → Schwab.
        let src_id = txn_uuid(txn_seq);
        txn_seq += 1;
        let tgt_id = txn_uuid(txn_seq);
        txn_seq += 1;
        let inv_date = date(y, m, 20);
        txns.push(Txn {
            id: src_id.clone(),
            account_id: ACC_CHECKING,
            date: inv_date,
            description: "SCHWAB ACH TRANSFER",
            amount: -1500.00,
            category: "transfer",
            subcategory: Some("investment_contribution"),
            merchant: "Charles Schwab",
            is_recurring: true,
            recurring_group: Some(RG_INVEST_ACH),
        });
        txns.push(Txn {
            id: tgt_id.clone(),
            account_id: ACC_SCHWAB,
            date: inv_date,
            description: "ACH DEPOSIT",
            amount: 1500.00,
            category: "investment",
            subcategory: Some("contribution"),
            merchant: "Charles Schwab",
            is_recurring: true,
            recurring_group: None,
        });
        flows.push(Flow {
            id: flow_uuid(flow_seq),
            source_account: ACC_CHECKING,
            target_account: ACC_SCHWAB,
            source_txn: src_id,
            target_txn: tgt_id,
            amount: 1500.00,
            flow_date: inv_date,
        });
        flow_seq += 1;

        // Regular checking → savings transfer — 5th — $500.
        let src_id = txn_uuid(txn_seq);
        txn_seq += 1;
        let tgt_id = txn_uuid(txn_seq);
        txn_seq += 1;
        let t_date = date(y, m, 5);
        txns.push(Txn {
            id: src_id.clone(),
            account_id: ACC_CHECKING,
            date: t_date,
            description: "TRANSFER TO SAVINGS",
            amount: -500.00,
            category: "transfer",
            subcategory: Some("savings"),
            merchant: "Chase Internal",
            is_recurring: false,
            recurring_group: None,
        });
        txns.push(Txn {
            id: tgt_id.clone(),
            account_id: ACC_SAVINGS,
            date: t_date,
            description: "TRANSFER FROM CHECKING",
            amount: 500.00,
            category: "transfer",
            subcategory: Some("savings"),
            merchant: "Chase Internal",
            is_recurring: false,
            recurring_group: None,
        });
        flows.push(Flow {
            id: flow_uuid(flow_seq),
            source_account: ACC_CHECKING,
            target_account: ACC_SAVINGS,
            source_txn: src_id,
            target_txn: tgt_id,
            amount: 500.00,
            flow_date: t_date,
        });
        flow_seq += 1;

        // Australia vacation boost — starting 2025-10 — extra $650/month.
        if date(y, m, 1) >= date(2025, 10, 1) {
            let src_id = txn_uuid(txn_seq);
            txn_seq += 1;
            let tgt_id = txn_uuid(txn_seq);
            txn_seq += 1;
            let t_date = date(y, m, 10);
            txns.push(Txn {
                id: src_id.clone(),
                account_id: ACC_CHECKING,
                date: t_date,
                description: "AUSTRALIA TRIP SAVINGS",
                amount: -650.00,
                category: "transfer",
                subcategory: Some("savings_goal"),
                merchant: "Chase Internal",
                is_recurring: false,
                recurring_group: None,
            });
            txns.push(Txn {
                id: tgt_id.clone(),
                account_id: ACC_SAVINGS,
                date: t_date,
                description: "TRANSFER FROM CHECKING",
                amount: 650.00,
                category: "transfer",
                subcategory: Some("savings_goal"),
                merchant: "Chase Internal",
                is_recurring: false,
                recurring_group: None,
            });
            flows.push(Flow {
                id: flow_uuid(flow_seq),
                source_account: ACC_CHECKING,
                target_account: ACC_SAVINGS,
                source_txn: src_id,
                target_txn: tgt_id,
                amount: 650.00,
                flow_date: t_date,
            });
            flow_seq += 1;
        }

        // Monthly subscriptions on Amex Reserve.
        for (day, desc, amt, merch, cat, sub, rg) in [
            (
                4u32,
                "NETFLIX.COM",
                15.99,
                "Netflix",
                "entertainment",
                "streaming",
                RG_NETFLIX,
            ),
            (
                4,
                "SPOTIFY USA",
                11.99,
                "Spotify",
                "entertainment",
                "streaming",
                RG_SPOTIFY,
            ),
            (
                12,
                "PLANET FITNESS",
                49.99,
                "Planet Fitness",
                "personal_care",
                "gym",
                RG_GYM,
            ),
        ] {
            let id = txn_uuid(txn_seq);
            txn_seq += 1;
            txns.push(Txn {
                id,
                account_id: ACC_AMEX,
                date: date(y, m, day),
                description: desc,
                amount: -amt,
                category: cat,
                subcategory: Some(sub),
                merchant: merch,
                is_recurring: true,
                recurring_group: Some(rg),
            });
        }
    }

    // -----------------------------------------------------------------
    // Variable spend: groceries, dining/coffee, gas, shopping, healthcare.
    // Sampled from clipped ranges. Distributed across Amex (dining, coffee)
    // and Atmos (shopping, gas) primarily, groceries on Joint Checking debit.
    // -----------------------------------------------------------------
    let groceries: &[&str] = &[
        "WHOLEFDS MKT #10234",
        "TRADER JOE'S #482",
        "COSTCO WHSE #1012",
        "SAFEWAY STORE 3321",
        "NEW SEASONS MARKET",
    ];
    let coffee: &[&str] = &[
        "STARBUCKS #4512",
        "STUMPTOWN COFFEE",
        "BLUE BOTTLE COFFEE",
        "PEETS COFFEE",
    ];
    let dining: &[&str] = &[
        "CHIPOTLE 1142",
        "PANERA BREAD",
        "OLIVE GARDEN",
        "SWEETGREEN",
        "MOD PIZZA",
        "PORTLAND PIZZA CO",
        "NEW SEASONS DELI",
    ];
    let gas: &[&str] = &[
        "SHELL OIL 57442",
        "CHEVRON #29841",
        "76 STATION",
        "ARCO AM/PM",
    ];
    let shopping: &[&str] = &[
        "AMZN MKTP US*RT4K2",
        "TARGET 00012345",
        "BEST BUY 1103",
        "HOME DEPOT 8844",
        "NIKE.COM",
        "REI OUTLET",
    ];
    let healthcare: &[&str] = &["CVS/PHARMACY #7422", "WALGREENS 11241", "KAISER PERMANENTE"];

    let mut d = start;
    while d <= end {
        // Groceries: ~1.3x/week on Joint Checking (debit).
        if rng.unit() < 0.20 {
            let id = txn_uuid(txn_seq);
            txn_seq += 1;
            let amt = rng.amount(60.0, 220.0);
            let m = *rng.choose(groceries);
            txns.push(Txn {
                id,
                account_id: ACC_CHECKING,
                date: d,
                description: m,
                amount: -amt,
                category: "food_dining",
                subcategory: Some("groceries"),
                merchant: m,
                is_recurring: false,
                recurring_group: None,
            });
        }
        // Coffee on Amex — 3x/week-ish.
        if rng.unit() < 0.42 {
            let id = txn_uuid(txn_seq);
            txn_seq += 1;
            let amt = rng.amount(4.50, 9.25);
            let m = *rng.choose(coffee);
            txns.push(Txn {
                id,
                account_id: ACC_AMEX,
                date: d,
                description: m,
                amount: -amt,
                category: "food_dining",
                subcategory: Some("coffee_shops"),
                merchant: m,
                is_recurring: false,
                recurring_group: None,
            });
        }
        // Dining on Amex — ~3x/week.
        if rng.unit() < 0.42 {
            let id = txn_uuid(txn_seq);
            txn_seq += 1;
            let amt = rng.amount(12.0, 68.0);
            let m = *rng.choose(dining);
            txns.push(Txn {
                id,
                account_id: ACC_AMEX,
                date: d,
                description: m,
                amount: -amt,
                category: "food_dining",
                subcategory: Some("restaurants"),
                merchant: m,
                is_recurring: false,
                recurring_group: None,
            });
        }
        // Gas on Atmos — ~1x/week.
        if rng.unit() < 0.14 {
            let id = txn_uuid(txn_seq);
            txn_seq += 1;
            let amt = rng.amount(35.0, 72.0);
            let m = *rng.choose(gas);
            txns.push(Txn {
                id,
                account_id: ACC_ATMOS,
                date: d,
                description: m,
                amount: -amt,
                category: "transportation",
                subcategory: Some("gas_fuel"),
                merchant: m,
                is_recurring: false,
                recurring_group: None,
            });
        }
        // Shopping on Atmos — ~1.5x/week.
        if rng.unit() < 0.22 {
            let id = txn_uuid(txn_seq);
            txn_seq += 1;
            let amt = rng.amount(15.0, 148.0);
            let m = *rng.choose(shopping);
            txns.push(Txn {
                id,
                account_id: ACC_ATMOS,
                date: d,
                description: m,
                amount: -amt,
                category: "shopping",
                subcategory: Some("general"),
                merchant: m,
                is_recurring: false,
                recurring_group: None,
            });
        }
        // Healthcare — rare (~2x/month).
        if rng.unit() < 0.06 {
            let id = txn_uuid(txn_seq);
            txn_seq += 1;
            let amt = rng.amount(22.0, 78.0);
            let m = *rng.choose(healthcare);
            txns.push(Txn {
                id,
                account_id: ACC_CHECKING,
                date: d,
                description: m,
                amount: -amt,
                category: "healthcare",
                subcategory: Some("copay"),
                merchant: m,
                is_recurring: false,
                recurring_group: None,
            });
        }

        d += Duration::days(1);
    }

    // -----------------------------------------------------------------
    // Credit card payoffs — once per month per card, 28th of month.
    // Amount = sum of that-month's charges on the card (rounded to $).
    // Paired transactions + account_flow. Category "transfer".
    // -----------------------------------------------------------------
    for mstart in &months {
        for (card_acc, card_desc) in [
            (ACC_AMEX, "AMEX RESERVE PAYMENT"),
            (ACC_ATMOS, "ATMOS VISA PAYMENT"),
        ] {
            let y = mstart.year();
            let m = mstart.month();
            // Sum charges within this month.
            let total: f64 = txns
                .iter()
                .filter(|t| {
                    t.account_id == card_acc
                        && t.date.year() == y
                        && t.date.month() == m
                        && t.amount < 0.0
                })
                .map(|t| -t.amount)
                .sum();
            let total = (total * 100.0).round() / 100.0;
            if total < 1.0 {
                continue;
            }
            let pay_date = date(y, m, 28);
            let src_id = txn_uuid(txn_seq);
            txn_seq += 1;
            let tgt_id = txn_uuid(txn_seq);
            txn_seq += 1;
            txns.push(Txn {
                id: src_id.clone(),
                account_id: ACC_CHECKING,
                date: pay_date,
                description: card_desc,
                amount: -total,
                category: "debt_payment",
                subcategory: Some("credit_card"),
                merchant: if card_acc == ACC_AMEX {
                    "American Express"
                } else {
                    "Atmos Visa"
                },
                is_recurring: false,
                recurring_group: None,
            });
            txns.push(Txn {
                id: tgt_id.clone(),
                account_id: card_acc,
                date: pay_date,
                description: "PAYMENT RECEIVED - THANK YOU",
                amount: total,
                category: "transfer",
                subcategory: Some("credit_card_payment"),
                merchant: if card_acc == ACC_AMEX {
                    "American Express"
                } else {
                    "Atmos Visa"
                },
                is_recurring: false,
                recurring_group: None,
            });
            flows.push(Flow {
                id: flow_uuid(flow_seq),
                source_account: ACC_CHECKING,
                target_account: card_acc,
                source_txn: src_id,
                target_txn: tgt_id,
                amount: total,
                flow_date: pay_date,
            });
            flow_seq += 1;
        }
    }

    // -----------------------------------------------------------------
    // Schwab brokerage gentle growth — quarterly dividend inflow.
    // -----------------------------------------------------------------
    for mstart in &months {
        if matches!(mstart.month(), 3 | 6 | 9 | 12) {
            let id = txn_uuid(txn_seq);
            txn_seq += 1;
            let amt = rng.amount(85.0, 140.0);
            txns.push(Txn {
                id,
                account_id: ACC_SCHWAB,
                date: date(mstart.year(), mstart.month(), 25),
                description: "DIVIDEND VTI",
                amount: amt,
                category: "investment",
                subcategory: Some("dividend"),
                merchant: "Vanguard ETF",
                is_recurring: false,
                recurring_group: None,
            });
        }
    }

    // -----------------------------------------------------------------
    // Sort transactions by date for readable SQL output. Stable for dedup
    // determinism: tiebreak by id.
    // -----------------------------------------------------------------
    txns.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));

    // -----------------------------------------------------------------
    // Emit SQL.
    // -----------------------------------------------------------------
    let sql = render_sql(&txns, &flows, &months);

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(&out_path, sql).expect("write sample.sql");

    eprintln!(
        "Wrote {} transactions, {} flows, {} months of budgets to {}",
        txns.len(),
        flows.len(),
        months.len(),
        out_path.display()
    );
}

// ---------------------------------------------------------------------------
// SQL rendering
// ---------------------------------------------------------------------------

fn render_sql(txns: &[Txn], flows: &[Flow], months: &[NaiveDate]) -> String {
    let mut out = String::new();
    out.push_str(
        "-- data/sample/sample.sql\n\
         --\n\
         -- Committed sample/demo dataset. NOT loaded in production.\n\
         -- Generated by: cargo run -p finima-api --bin finima-generate-sample\n\
         -- Load: make sample-load\n\
         -- Purge: make sample-purge\n\
         --\n\
         -- Fixed UUIDs so reruns are idempotent via ON CONFLICT DO NOTHING.\n\
         -- Portfolio: b2000000-0000-4000-8000-000000000001 (Sample Household)\n\
         -- User:      a2000000-0000-4000-8000-000000000001 (sample@finima.local)\n\n",
    );

    // User + portfolio.
    out.push_str(&format!(
        "INSERT INTO users (id, email, display_name, created_at, updated_at)\n\
         VALUES ({}, 'sample@finima.local', 'Sample Household', NOW(), NOW())\n\
         ON CONFLICT (id) DO NOTHING;\n\n",
        sql_str(USER_ID)
    ));
    out.push_str(&format!(
        "INSERT INTO portfolios (id, user_id, name, created_at)\n\
         VALUES ({}, {}, 'Sample Household', NOW())\n\
         ON CONFLICT (id) DO NOTHING;\n\n",
        sql_str(PORTFOLIO_ID),
        sql_str(USER_ID)
    ));

    // Accounts.
    out.push_str("-- Accounts\n");
    let accounts: [(&str, &str, &str, &str, f64, bool); 6] = [
        (
            ACC_CHECKING,
            "Joint Checking",
            "checking",
            "Chase Bank",
            8500.00,
            true,
        ),
        (
            ACC_SAVINGS,
            "Joint Savings",
            "savings",
            "Chase Bank",
            24000.00,
            false,
        ),
        (
            ACC_AMEX,
            "Amex Reserve",
            "credit_card",
            "American Express",
            0.00,
            false,
        ),
        (ACC_ATMOS, "Atmos Visa", "credit_card", "Atmos", 0.00, false),
        (
            ACC_MAZDA,
            "Mazda CX-90 Loan",
            "loan_auto",
            "Mazda Financial",
            -38000.00,
            false,
        ),
        (
            ACC_SCHWAB,
            "Schwab Brokerage",
            "investment_brokerage",
            "Charles Schwab",
            65000.00,
            false,
        ),
    ];
    for (id, name, ty, inst, ob, primary) in accounts {
        out.push_str(&format!(
            "INSERT INTO accounts (id, portfolio_id, name, institution, account_type, opening_balance, is_primary_income, created_at)\n\
             VALUES ({}, {}, {}, {}, {}, {:.2}, {}, NOW())\n\
             ON CONFLICT (id) DO NOTHING;\n",
            sql_str(id),
            sql_str(PORTFOLIO_ID),
            sql_str(name),
            sql_str(inst),
            sql_str(ty),
            ob,
            primary
        ));
    }
    out.push('\n');

    // Recurring groups.
    out.push_str("-- Recurring groups\n");
    let rgs: [(&str, &str, &str, &str, f64); 11] = [
        (
            RG_PAYROLL,
            "Employer Payroll",
            "income",
            "biweekly",
            6400.00,
        ),
        (RG_RENT, "Landlord ACH", "housing", "monthly", -2900.00),
        (
            RG_ELECTRIC,
            "Pacific Power",
            "utilities",
            "monthly",
            -120.00,
        ),
        (RG_GAS_UTIL, "NW Natural", "utilities", "monthly", -45.00),
        (RG_INTERNET, "Comcast", "utilities", "monthly", -89.99),
        (RG_WATER, "City Water", "utilities", "monthly", -45.00),
        (RG_NETFLIX, "Netflix", "entertainment", "monthly", -15.99),
        (RG_SPOTIFY, "Spotify", "entertainment", "monthly", -11.99),
        (RG_GYM, "Planet Fitness", "personal_care", "monthly", -49.99),
        (
            RG_MAZDA_PMT,
            "Mazda Financial",
            "debt_payment",
            "monthly",
            -625.00,
        ),
        (
            RG_INVEST_ACH,
            "Charles Schwab",
            "transfer",
            "monthly",
            -1500.00,
        ),
    ];
    for (id, merch, cat, freq, avg) in rgs {
        out.push_str(&format!(
            "INSERT INTO recurring_groups (id, portfolio_id, merchant_name, category, frequency, avg_amount, is_confirmed, metadata)\n\
             VALUES ({}, {}, {}, {}, {}, {:.2}, true, '{{}}')\n\
             ON CONFLICT (id) DO NOTHING;\n",
            sql_str(id),
            sql_str(PORTFOLIO_ID),
            sql_str(merch),
            sql_str(cat),
            sql_str(freq),
            avg
        ));
    }
    out.push('\n');

    // Transactions — batched.
    out.push_str("-- Transactions\n");
    for chunk in txns.chunks(100) {
        out.push_str(
            "INSERT INTO transactions (id, account_id, date, amount, description, original_description, category, subcategory, merchant_name, is_recurring, recurring_group_id, direction, dedup_hash, created_at) VALUES\n",
        );
        let mut parts: Vec<String> = Vec::with_capacity(chunk.len());
        for (i, t) in chunk.iter().enumerate() {
            let dedup = format!("sample-{}-{}", &t.account_id[29..], i);
            parts.push(format!(
                "  ({}, {}, '{}', {:.2}, {}, {}, {}, {}, {}, {}, {}, {}, {}, NOW())",
                sql_str(&t.id),
                sql_str(t.account_id),
                t.date,
                t.amount,
                sql_str(t.description),
                sql_str(t.description),
                sql_str(t.category),
                sql_opt_str(t.subcategory),
                sql_str(t.merchant),
                t.is_recurring,
                match t.recurring_group {
                    Some(g) => sql_str(g),
                    None => "NULL".to_string(),
                },
                sql_str(t.direction()),
                sql_str(&format!("{}-{}", &t.id[24..], dedup)),
            ));
        }
        out.push_str(&parts.join(",\n"));
        out.push_str("\nON CONFLICT (id) DO NOTHING;\n\n");
    }

    // Account flows.
    out.push_str("-- Account flows\n");
    for f in flows {
        out.push_str(&format!(
            "INSERT INTO account_flows (id, portfolio_id, source_account_id, target_account_id, source_transaction_id, target_transaction_id, amount, flow_date, is_auto_detected, is_confirmed, created_at)\n\
             VALUES ({}, {}, {}, {}, {}, {}, {:.2}, '{}', false, true, NOW())\n\
             ON CONFLICT (id) DO NOTHING;\n",
            sql_str(&f.id),
            sql_str(PORTFOLIO_ID),
            sql_str(f.source_account),
            sql_str(f.target_account),
            sql_str(&f.source_txn),
            sql_str(&f.target_txn),
            f.amount,
            f.flow_date
        ));
    }
    out.push('\n');

    // Budgets — per category × month, all 18 months.
    out.push_str("-- Budgets (6 categories × 18 months)\n");
    let cats: &[(&str, f64)] = &[
        ("food_dining", 900.00),
        ("transportation", 400.00),
        ("shopping", 350.00),
        ("entertainment", 150.00),
        ("utilities", 350.00),
        ("healthcare", 120.00),
    ];
    let mut budget_seq: u32 = 1;
    for mstart in months {
        for (cat, lim) in cats {
            let bid = format!("b3000000-0000-4000-8000-{:012x}", budget_seq as u64);
            budget_seq += 1;
            out.push_str(&format!(
                "INSERT INTO budgets (id, portfolio_id, category, monthly_limit, rollover, month)\n\
                 VALUES ({}, {}, {}, {:.2}, false, '{}')\n\
                 ON CONFLICT DO NOTHING;\n",
                sql_str(&bid),
                sql_str(PORTFOLIO_ID),
                sql_str(cat),
                lim,
                mstart
            ));
        }
    }
    out.push('\n');

    // Savings goal — Australia round-trip.
    out.push_str("-- Savings goal: Australia Round-Trip\n");
    out.push_str(&format!(
        "INSERT INTO savings_goals (id, portfolio_id, name, target_amount, current_amount, target_date, linked_account_id)\n\
         VALUES ({}, {}, 'Australia Round-Trip', 14000.00, 4550.00, '2026-11-15', {})\n\
         ON CONFLICT (id) DO NOTHING;\n",
        sql_str(SAVINGS_GOAL_ID),
        sql_str(PORTFOLIO_ID),
        sql_str(ACC_SAVINGS)
    ));

    out
}
