# Finima — Wireframes

**Version:** 1.0 | **Date:** 2026-04-10

All wireframes are low-fidelity ASCII representations of the key screens. These define layout structure, content hierarchy, and interaction zones — not visual design.

---

## W-01: Sign-In Page

```text
╔══════════════════════════════════════════════════════════╗
║                                                          ║
║                      ╭──────────╮                        ║
║                      │  FINIMA  │                        ║
║                      │   logo   │                        ║
║                      ╰──────────╯                        ║
║                                                          ║
║            Your finances, your intelligence.              ║
║                                                          ║
║         ┌──────────────────────────────────┐             ║
║         │  Email address                   │             ║
║         └──────────────────────────────────┘             ║
║                                                          ║
║         ┌──────────────────────────────────┐             ║
║         │     ✉  Send Magic Link           │             ║
║         └──────────────────────────────────┘             ║
║                                                          ║
║         No password needed. We'll email you              ║
║         a secure sign-in link.                           ║
║                                                          ║
╚══════════════════════════════════════════════════════════╝
```

---

## W-02: Magic Link Sent Confirmation

```text
╔══════════════════════════════════════════════════════════╗
║                                                          ║
║                      ╭──────────╮                        ║
║                      │    ✉     │                        ║
║                      ╰──────────╯                        ║
║                                                          ║
║              Check your email                            ║
║                                                          ║
║       We sent a sign-in link to                          ║
║       sarah@example.com                                  ║
║                                                          ║
║       Click the link to sign in.                         ║
║       It expires in 15 minutes.                          ║
║                                                          ║
║       ┌──────────────────────────────────┐               ║
║       │   Didn't receive it? Resend      │               ║
║       └──────────────────────────────────┘               ║
║                                                          ║
╚══════════════════════════════════════════════════════════╝
```

---

## W-03: Onboarding Wizard

```text
╔══════════════════════════════════════════════════════════════════╗
║  FINIMA                                    Step 2 of 3          ║
╠══════════════════════════════════════════════════════════════════╣
║                                                                  ║
║   ●────────●────────○                                            ║
║   Profile   Portfolio  Account                                   ║
║                                                                  ║
║   ┌────────────────────────────────────────────────────────┐     ║
║   │  Create Your Portfolio                                  │     ║
║   │                                                         │     ║
║   │  A portfolio groups all your accounts together.         │     ║
║   │                                                         │     ║
║   │  Name*  ┌──────────────────────────────────┐            │     ║
║   │         │ My Finances                       │            │     ║
║   │         └──────────────────────────────────┘            │     ║
║   │                                                         │     ║
║   │  Description (optional)                                 │     ║
║   │         ┌──────────────────────────────────┐            │     ║
║   │         │                                   │            │     ║
║   │         └──────────────────────────────────┘            │     ║
║   │                                                         │     ║
║   │         ┌──────────┐  ┌──────────┐                      │     ║
║   │         │   Back   │  │   Next   │                      │     ║
║   │         └──────────┘  └──────────┘                      │     ║
║   └────────────────────────────────────────────────────────┘     ║
╚══════════════════════════════════════════════════════════════════╝
```

---

## W-04: Main Dashboard

```text
╔══════════════════════════════════════════════════════════════════════════════╗
║  FINIMA          Dashboard   Accounts   Transactions   Budget   News    ⚙  ║
╠═══════════╦══════════════════════════════════════════════════════════════════╣
║           ║                                                                 ║
║  SIDEBAR  ║  ┌─────────────────────────┐  ┌─────────────────────────┐      ║
║           ║  │   NET WORTH             │  │   FINANCIAL HEALTH      │      ║
║  Dashboard║  │   $127,400              │  │   Score: 72/100         │      ║
║  Accounts ║  │   ▲ $2,100 this month   │  │   ████████░░  Good     │      ║
║  Transact.║  │   [LINE CHART ~~~~~~~~] │  │   Savings rate: 15%    │      ║
║  Recurring║  │                         │  │   Debt ratio: 22%      │      ║
║  Budget   ║  └─────────────────────────┘  └─────────────────────────┘      ║
║  Goals    ║                                                                 ║
║  News     ║  ┌─────────────────────────┐  ┌─────────────────────────┐      ║
║           ║  │   CASH FLOW             │  │   SPENDING BY CATEGORY  │      ║
║  ─────────║  │                         │  │                         │      ║
║  Settings ║  │   [BAR CHART]           │  │   [DONUT CHART]         │      ║
║  ⚙ Prefs  ║  │   ██ Inc  ░░ Exp       │  │   ◉ Housing    38%     │      ║
║           ║  │   Jan Feb Mar Apr May   │  │   ◉ Food       15%     │      ║
║           ║  │                         │  │   ◉ Transport  10%     │      ║
║           ║  └─────────────────────────┘  │   ◉ Shopping    8%     │      ║
║           ║                               │   ◉ Other      29%     │      ║
║           ║  ┌─────────────────────────┐  └─────────────────────────┘      ║
║           ║  │   UPCOMING BILLS        │                                    ║
║           ║  │                         │  ┌─────────────────────────┐      ║
║           ║  │   Apr 15  Netflix $15.99│  │   BUDGET vs ACTUAL      │      ║
║           ║  │   Apr 18  Spotify  $9.99│  │                         │      ║
║           ║  │   Apr 20  Internet $65  │  │   Food     ████████░ 80%│      ║
║           ║  │   May 01  Rent   $1,800 │  │   Shop     ████████████ 120%│  ║
║           ║  │                         │  │   Trans    ██████░░░ 60% │     ║
║           ║  └─────────────────────────┘  └─────────────────────────┘      ║
╚═══════════╩════════════════════════════════════════════════════════════════╝
```

**Layout notes:** Dashboard widgets are rearrangeable via drag-and-drop (`react-grid-layout`). Users save their preferred arrangement. Each widget has a ⋮ menu for resize/remove/configure.

---

## W-05: Accounts List

```text
╔══════════════════════════════════════════════════════════════════════╗
║  Accounts                                        [+ Add Account]   ║
╠══════════════════════════════════════════════════════════════════════╣
║                                                                      ║
║  Portfolio: My Finances ▼                                            ║
║                                                                      ║
║  ┌───────────────────────────────────────────────────────────────┐   ║
║  │  🏦 Chase Checking          Checking     $3,247.82           │   ║
║  │     Chase Bank · Last import: Apr 8, 2026 · 342 transactions │   ║
║  ├───────────────────────────────────────────────────────────────┤   ║
║  │  💰 Ally Savings            Savings      $12,400.00          │   ║
║  │     Ally Bank · Last import: Apr 5, 2026 · 45 transactions   │   ║
║  ├───────────────────────────────────────────────────────────────┤   ║
║  │  💳 Amex Gold               Credit Card  -$1,283.50          │   ║
║  │     American Express · Last import: Apr 7, 2026 · 89 txns   │   ║
║  ├───────────────────────────────────────────────────────────────┤   ║
║  │  📈 Fidelity 401(k)         Retirement   $45,200.00          │   ║
║  │     Fidelity · Last import: Mar 30, 2026 · 24 transactions   │   ║
║  ├───────────────────────────────────────────────────────────────┤   ║
║  │  🏠 Mortgage                Loan         -$187,500.00        │   ║
║  │     Wells Fargo · Last import: Apr 1, 2026 · 12 transactions │   ║
║  └───────────────────────────────────────────────────────────────┘   ║
║                                                                      ║
║  Total Assets: $60,847.82    Total Liabilities: -$188,783.50        ║
║  Net Worth: -$127,935.68                                             ║
╚══════════════════════════════════════════════════════════════════════╝
```

---

## W-06: Transaction Table (Aggregate View)

```text
╔═══════════════════════════════════════════════════════════════════════════════╗
║  Transactions                                                                ║
╠═══════════════════════════════════════════════════════════════════════════════╣
║  Filters: [Date Range ▼] [Account ▼] [Category ▼] [🔍 Search...        ]   ║
║           [Amount min] — [Amount max]    [Clear Filters]                      ║
║                                                                               ║
║  Showing 342 transactions · Selected: 0 · [Bulk Edit ▼] [Export CSV]         ║
║                                                                               ║
║  ┌──┬───────────┬──────────────────────┬──────────────┬────────┬──────────┐  ║
║  │☐ │ Date      │ Description          │ Category     │ Amount │ Account  │  ║
║  ├──┼───────────┼──────────────────────┼──────────────┼────────┼──────────┤  ║
║  │☐ │ Apr 08    │ WHOLEFDS MKT #10432  │ Food > Groc  │ -$87.42│ Chase Ck │  ║
║  │☐ │ Apr 07    │ SHELL OIL 57442      │ Transport>Gas│ -$52.00│ Amex Gold│  ║
║  │☐ │ Apr 07    │ SPOTIFY USA          │ Entertain>Sub│  -$9.99│ Chase Ck │  ║
║  │☐ │ Apr 06    │ EMPLOYER DIRECT DEP  │ Income>Salary│+$4,100 │ Chase Ck │  ║
║  │☐ │ Apr 05    │ AMZN*3829481         │ ⚠️ Other     │ -$34.99│ Amex Gold│  ║
║  │  │           │                      │ [click to fix]│        │          │  ║
║  │☐ │ Apr 04    │ TRANSFER TO SAVINGS  │ Transfer     │-$500.00│ Chase Ck │  ║
║  │☐ │ Apr 03    │ SQ *GREENLEAF CAFE   │ Food > Rest  │ -$18.50│ Chase Ck │  ║
║  └──┴───────────┴──────────────────────┴──────────────┴────────┴──────────┘  ║
║                                                                               ║
║  ◀ 1 2 3 ... 12 ▶   Showing 1-30 of 342                                     ║
╚═══════════════════════════════════════════════════════════════════════════════╝
```

**Interaction notes:**

- ⚠️ icon indicates low-confidence AI categorization; clicking opens a dropdown editor
- Checkbox selection enables Bulk Edit toolbar
- Column headers are sortable (click to toggle asc/desc)
- Category cells are inline-editable with autocomplete dropdown

---

## W-07: File Upload & Column Mapping (CSV)

```text
╔══════════════════════════════════════════════════════════════════════╗
║  Import Transactions → Chase Checking                               ║
╠══════════════════════════════════════════════════════════════════════╣
║                                                                      ║
║  ┌──────────────────────────────────────────────────────────────┐   ║
║  │                                                              │   ║
║  │              ┌─────────────────────┐                         │   ║
║  │              │     📁 Drop file    │                         │   ║
║  │              │   or click to browse│                         │   ║
║  │              │                     │                         │   ║
║  │              │  CSV·OFX·QFX·QIF·XLS│                         │   ║
║  │              └─────────────────────┘                         │   ║
║  │                                                              │   ║
║  └──────────────────────────────────────────────────────────────┘   ║
║                                                                      ║
║  File: chase_march_2026.csv (48 KB)                                  ║
║                                                                      ║
║  Column Mapping:                                                     ║
║  ┌──────────────────┬────────────────────────────────────────────┐   ║
║  │ File Column      │ Maps To                                    │   ║
║  ├──────────────────┼────────────────────────────────────────────┤   ║
║  │ "Transaction Date│ [Date ▼]                                   │   ║
║  │ "Description"    │ [Description ▼]                            │   ║
║  │ "Amount"         │ [Amount ▼]                                 │   ║
║  │ "Balance"        │ [-- Skip -- ▼]                             │   ║
║  │ "Category"       │ [Category (optional) ▼]                    │   ║
║  └──────────────────┴────────────────────────────────────────────┘   ║
║                                                                      ║
║  Preview (first 5 rows):                                             ║
║  ┌───────────┬────────────────────────┬──────────┬──────────┐       ║
║  │ Date      │ Description            │ Amount   │ Balance  │       ║
║  ├───────────┼────────────────────────┼──────────┼──────────┤       ║
║  │ 03/01/2026│ WHOLEFDS MKT #10432    │ -87.42   │ 3,160.40 │       ║
║  │ 03/02/2026│ SHELL OIL 57442        │ -52.00   │ 3,108.40 │       ║
║  │ 03/03/2026│ EMPLOYER DIRECT DEP    │ +4100.00 │ 7,208.40 │       ║
║  └───────────┴────────────────────────┴──────────┴──────────┘       ║
║                                                                      ║
║  Date format detected: MM/DD/YYYY                                    ║
║  ☑ Skip duplicate transactions (by date + amount + description)      ║
║                                                                      ║
║  ┌───────────────────┐                                               ║
║  │  Import 127 rows  │                                               ║
║  └───────────────────┘                                               ║
╚══════════════════════════════════════════════════════════════════════╝
```

---

## W-08: Recurring Payments Page

```text
╔══════════════════════════════════════════════════════════════════════╗
║  Recurring Payments & Income                                        ║
╠══════════════════════════════════════════════════════════════════════╣
║                                                                      ║
║  ┌─── CONFIRMED ───────────────────────────────────────────────┐    ║
║  │ Merchant          │ Amount    │ Frequency │ Annual  │ Action │    ║
║  ├───────────────────┼───────────┼───────────┼─────────┼────────┤    ║
║  │ Netflix           │ $15.99/mo │ Monthly   │ $191.88 │ ⋮      │    ║
║  │ Spotify           │  $9.99/mo │ Monthly   │ $119.88 │ ⋮      │    ║
║  │ Internet (Xfinity)│ $65.00/mo │ Monthly   │ $780.00 │ ⋮      │    ║
║  │ Rent              │$1,800/mo  │ Monthly   │$21,600  │ ⋮      │    ║
║  │ Car Insurance     │ $145/qtr  │ Quarterly │ $580.00 │ ⋮      │    ║
║  │ Employer Payroll  │+$4,100/mo │ Biweekly  │$98,400  │ ⋮      │    ║
║  └───────────────────┴───────────┴───────────┴─────────┴────────┘    ║
║  Total recurring expenses: $2,035.98/mo · $24,431.76/yr             ║
║  Total recurring income:   $4,100.00/mo · $98,400.00/yr             ║
║                                                                      ║
║  ┌─── PENDING REVIEW (3) ──────────────────────────────────────┐    ║
║  │ Disney+            │ $13.99/mo │ Monthly   │ $167.88│ ✓  ✕  │    ║
║  │ Costco Membership  │ $65/yr    │ Annual    │  $65.00│ ✓  ✕  │    ║
║  │ Planet Fitness     │ $25/mo    │ Monthly   │ $300.00│ ✓  ✕  │    ║
║  └────────────────────┴───────────┴───────────┴────────┴───────┘    ║
╚══════════════════════════════════════════════════════════════════════╝
```

---

## W-09: Budget Page

```text
╔══════════════════════════════════════════════════════════════════════╗
║  Budget — April 2026                     [◀ Mar] [Apr ▼] [May ▶]   ║
╠══════════════════════════════════════════════════════════════════════╣
║                                                                      ║
║  [Auto-Suggest Budget]                                               ║
║                                                                      ║
║  Category          │ Budget  │ Spent   │ Remaining │ Progress       ║
║  ──────────────────┼─────────┼─────────┼───────────┼───────────     ║
║  🏠 Housing        │ $2,100  │ $1,865  │    $235   │ ████████░ 89%  ║
║  🍔 Food & Dining  │   $400  │   $312  │     $88   │ ██████░░░ 78%  ║
║  🚗 Transportation │   $300  │   $195  │    $105   │ █████░░░░ 65%  ║
║  🛍️ Shopping       │   $500  │   $520  │    -$20   │ ██████████ 104%║
║  🎬 Entertainment  │   $150  │    $89  │     $61   │ ████░░░░░ 59%  ║
║  🏥 Healthcare     │   $200  │     $0  │    $200   │ ░░░░░░░░░  0%  ║
║  ──────────────────┼─────────┼─────────┼───────────┼───────────     ║
║  TOTAL             │ $3,650  │ $2,981  │    $669   │ ███████░░ 82%  ║
║                                                                      ║
║  ┌─ SAVINGS GOALS ─────────────────────────────────────────────┐    ║
║  │ 🎯 Emergency Fund    $12,000 / $15,000   ████████░░ 80%    │    ║
║  │    Projected completion: Jul 2026                            │    ║
║  │                                                              │    ║
║  │ ✈️ Vacation Fund     $800 / $3,000       ██░░░░░░░░ 27%    │    ║
║  │    Projected completion: Feb 2027                            │    ║
║  └──────────────────────────────────────────────────────────────┘    ║
╚══════════════════════════════════════════════════════════════════════╝
```

---

## W-10: Preferences / Settings

```text
╔══════════════════════════════════════════════════════════════════════╗
║  Settings                                                            ║
╠══════════════════════════════════════════════════════════════════════╣
║                                                                      ║
║  [Theme]  [Layout]  [General]  [LLM]                                 ║
║                                                                      ║
║  ── Theme ─────────────────────────────────────────────────────      ║
║                                                                      ║
║  Mode:    (●) Light   (○) Dark   (○) System                         ║
║                                                                      ║
║  Accent Color:  [██ #3B82F6] [Pick...]                               ║
║                                                                      ║
║  Preview:  ┌─────────────────────────────┐                           ║
║            │  Sample card with accent    │                           ║
║            │  [Primary Button]           │                           ║
║            └─────────────────────────────┘                           ║
║                                                                      ║
║  ── Layout (Dashboard Widgets) ────────────────────────────────      ║
║                                                                      ║
║  ☑ Net Worth         ☑ Cash Flow                                     ║
║  ☑ Spending Breakdown ☑ Budget vs Actual                             ║
║  ☑ Upcoming Bills     ☑ Financial Health                             ║
║  ☐ Savings Goals      ☐ Recent Transactions                         ║
║                                                                      ║
║  [Reset to Default Layout]                                           ║
║                                                                      ║
║  ── General ───────────────────────────────────────────────────      ║
║                                                                      ║
║  Currency:       [USD - $ ▼]                                         ║
║  Date format:    [MM/DD/YYYY ▼]                                      ║
║  Fiscal month:   [January ▼]                                         ║
║  Default chart:  [Bar Chart ▼]                                       ║
║                                                                      ║
║  ── LLM Configuration ────────────────────────────────────────      ║
║                                                                      ║
║  Provider:   (●) Ollama   (○) llama.cpp direct                       ║
║  Model:      [gemma-4-26b-a4b-it ▼]                                  ║
║  Endpoint:   [http://localhost:11434    ]                             ║
║  Status:     🟢 Connected (Gemma 4 26B loaded)                       ║
║                                                                      ║
║  [Save Changes]                                                      ║
╚══════════════════════════════════════════════════════════════════════╝
```

---

## W-11: Money Flow — Sankey Diagram + Outflow Ranking

```text
╔══════════════════════════════════════════════════════════════════════════════╗
║  Money Flow                                    April 2026 [◀] [▼] [▶]      ║
╠══════════════════════════════════════════════════════════════════════════════╣
║                                                                              ║
║  [Sankey]  [Balance Impact]  [Flow Groups]                                   ║
║                                                                              ║
║  ┌────────────────────────────────────────────────────────────────────────┐  ║
║  │                                                                        │  ║
║  │    INCOME                          OUTFLOWS                            │  ║
║  │                                                                        │  ║
║  │  ┌──────────┐    ════════════════▶ Mortgage        $1,800              │  ║
║  │  │          │    ═══════════▶       Amex Gold         $650              │  ║
║  │  │  Chase   │    ════════▶         Ally Savings       $500              │  ║
║  │  │ Checking │    ══════▶           Student Loan       $280              │  ║
║  │  │ $4,100   │    ════▶             Car Insurance      $145              │  ║
║  │  │          │    ═══▶              Discretionary      $725              │  ║
║  │  └──────────┘                                                          │  ║
║  │                                                                        │  ║
║  │  ┌──────────┐    ════════▶         Joint Savings      $400              │  ║
║  │  │  Ally    │    ══════▶           Groceries CC       $350              │  ║
║  │  │ Checking │    ═════▶            Discretionary      $550              │  ║
║  │  │ $1,300   │                                                          │  ║
║  │  └──────────┘                                                          │  ║
║  │                                                                        │  ║
║  └────────────────────────────────────────────────────────────────────────┘  ║
║                                                                              ║
║  ── OUTFLOW RANKING (from primary accounts) ──────────────────────────────  ║
║                                                                              ║
║  ┌──────────────────┬──────────┬──────────┬──────────┬────────┬──────────┐  ║
║  │ Account          │ Type     │ Monthly  │ % Income │ Trend  │ Action   │  ║
║  ├──────────────────┼──────────┼──────────┼──────────┼────────┼──────────┤  ║
║  │ Mortgage         │ Loan     │ $1,800   │ 33.3%    │ →      │ View     │  ║
║  │ Amex Gold        │ Credit   │   $650   │ 12.0%    │ ↑ 25%  │ View     │  ║
║  │ Ally Savings     │ Savings  │   $500   │  9.3%    │ →      │ View     │  ║
║  │ Joint Savings    │ Savings  │   $400   │  7.4%    │ →      │ View     │  ║
║  │ Groceries CC     │ Credit   │   $350   │  6.5%    │ ↓  8%  │ View     │  ║
║  │ Student Loan     │ Loan     │   $280   │  5.2%    │ →      │ View     │  ║
║  │ Car Insurance    │ Loan     │   $145   │  2.7%    │ →      │ View     │  ║
║  ├──────────────────┼──────────┼──────────┼──────────┼────────┼──────────┤  ║
║  │ TOTAL OUTFLOWS   │          │ $4,125   │ 76.4%    │        │          │  ║
║  │ DISCRETIONARY    │          │ $1,275   │ 23.6%    │        │          │  ║
║  └──────────────────┴──────────┴──────────┴──────────┴────────┴──────────┘  ║
║                                                                              ║
║  💡 Insight: "Amex Gold outflow increased 25% over 3 months, driven by      ║
║     rising dining ($+120) and travel ($+80) spending."                       ║
║                                                                              ║
╚══════════════════════════════════════════════════════════════════════════════╝
```

---

## W-12: Money Flow — Balance Impact Waterfall

```text
╔══════════════════════════════════════════════════════════════════════════════╗
║  Money Flow > Balance Impact                   April 2026 [◀] [▼] [▶]      ║
╠══════════════════════════════════════════════════════════════════════════════╣
║                                                                              ║
║  [Sankey]  [Balance Impact]  [Flow Groups]                                   ║
║                                                                              ║
║  Account: [Chase Checking ▼]                                                 ║
║                                                                              ║
║  ┌────────────────────────────────────────────────────────────────────────┐  ║
║  │                                                                        │  ║
║  │  $8,000 ┤                                                              │  ║
║  │         │         ┌─────┐                                              │  ║
║  │  $7,000 ┤         │     │                                              │  ║
║  │         │         │+4100│                                              │  ║
║  │  $6,000 ┤         │     │                                              │  ║
║  │         │         │     │                                              │  ║
║  │  $5,000 ┤         │     ├─────┐                                        │  ║
║  │         │         │     │-1800│                                        │  ║
║  │  $4,000 ┤  ┌─────┐│     │     ├────┐                                   │  ║
║  │         │  │Start ││     │     │-650├───┐                               │  ║
║  │  $3,000 ┤  │$3200││     │     │    │-500├──┐                           │  ║
║  │         │  │     ││     │     │    │   │-280├─┐                        │  ║
║  │  $2,000 ┤  │     ││     │     │    │   │   │  ├─┐  ┌─────┐            │  ║
║  │         │  │     ││     │     │    │   │   │  │ │  │ End │            │  ║
║  │  $1,000 ┤  │     ││     │     │    │   │   │  │ │  │$3925│            │  ║
║  │         │  │     ││     │     │    │   │   │  │ │  │     │            │  ║
║  │      $0 ┤──┴─────┴┴─────┴─────┴────┴───┴───┴──┴─┴──┴─────┴──          │  ║
║  │          Start  Income  Mortg  Amex Saving Loan Ins  Other  End        │  ║
║  │                                                                        │  ║
║  └────────────────────────────────────────────────────────────────────────┘  ║
║                                                                              ║
║  Summary: Started at $3,200 → received $4,100 income → paid out $4,375     ║
║  → ended at $3,925 (net +$725 for the month)                                ║
║                                                                              ║
║  Largest outflow: Mortgage ($1,800 · 43.9% of income)                       ║
║  Fastest growing: Amex Gold (↑25% over 3 months)                            ║
║                                                                              ║
╚══════════════════════════════════════════════════════════════════════════════╝
```

---

## W-13: Account Settings — Primary Income Toggle

```text
╔══════════════════════════════════════════════════════════════════════╗
║  Edit Account — Chase Checking                                       ║
╠══════════════════════════════════════════════════════════════════════╣
║                                                                      ║
║  Name        ┌──────────────────────────────────┐                    ║
║              │ Chase Checking                    │                    ║
║              └──────────────────────────────────┘                    ║
║                                                                      ║
║  Institution ┌──────────────────────────────────┐                    ║
║              │ Chase Bank                        │                    ║
║              └──────────────────────────────────┘                    ║
║                                                                      ║
║  Type        [Checking ▼]                                            ║
║                                                                      ║
║  Currency    [USD - $ ▼]                                             ║
║                                                                      ║
║  ── Income Tracking ──────────────────────────────────────────       ║
║                                                                      ║
║  Primary Income Account   [████ ON ]                                 ║
║                                                                      ║
║  ℹ️ Mark this account as a primary source of income (e.g., where     ║
║  your paycheck is deposited). Finima will track how money flows      ║
║  from this account to your other accounts — credit cards, loans,     ║
║  savings — so you can see where every dollar goes.                   ║
║                                                                      ║
║  ── Danger Zone ──────────────────────────────────────────────       ║
║                                                                      ║
║  [Archive Account]                                                   ║
║                                                                      ║
║  ┌───────────────────┐                                               ║
║  │   Save Changes    │                                               ║
║  └───────────────────┘                                               ║
╚══════════════════════════════════════════════════════════════════════╝
```
