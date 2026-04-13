# Finima — User Journeys

**Version:** 1.0 | **Date:** 2026-04-10

---

## Journey 1: First-Time User — "Getting Started"

**Persona:** Sarah, a DIY budgeter who just heard about Finima  
**Goal:** Sign up and see her first month of spending categorized  
**Duration:** ~10 minutes

```text
┌─────────────────────────────────────────────────────────────────────┐
│  LANDING PAGE                                                       │
│  Sarah enters her email → clicks "Get Started"                      │
└──────────────────────┬──────────────────────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  EMAIL INBOX                                                        │
│  Sarah receives branded email with magic link                       │
│  Clicks "Sign in to Finima" button                                  │
└──────────────────────┬──────────────────────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  ONBOARDING — STEP 1: Profile                                       │
│  Enters display name: "Sarah"                                       │
│  Selects: USD, MM/DD/YYYY                                           │
│  → clicks Next                                                      │
└──────────────────────┬──────────────────────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  ONBOARDING — STEP 2: Portfolio                                      │
│  Names it: "My Finances"                                            │
│  → clicks Next                                                      │
└──────────────────────┬──────────────────────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  ONBOARDING — STEP 3: First Account                                  │
│  Type: Checking                                                      │
│  Name: "Chase Checking"                                              │
│  Institution: "Chase"                                                │
│  Opening balance: $3,200.00                                          │
│  → clicks Create Account                                            │
└──────────────────────┬──────────────────────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  ACCOUNT VIEW — Upload Prompt                                        │
│  "Import your first transactions!" CTA                               │
│  Sarah logs into chase.com, downloads last month as CSV              │
│  Drags the CSV file into the upload zone                             │
└──────────────────────┬──────────────────────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  COLUMN MAPPING                                                      │
│  System shows preview: "Date | Description | Amount | Balance"       │
│  Sarah confirms: col1=Date, col2=Description, col3=Amount            │
│  → clicks Import                                                     │
└──────────────────────┬──────────────────────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  IMPORT PROGRESS                                                     │
│  Progress bar: "Importing 127 transactions..."                       │
│  Then: "Categorizing with AI... 45%... 80%... Done!"                │
│  3 flagged for review (low confidence)                               │
└──────────────────────┬──────────────────────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│  DASHBOARD                                                           │
│  Sarah sees: spending donut chart, cash flow bar, balance over time  │
│  🎉 "You spent $1,847 this month. Top: Food ($412), Housing ($950)"  │
│  Emotion: Satisfaction — "That was easy!"                            │
└─────────────────────────────────────────────────────────────────────┘
```

**Key Moments:**

- ✅ Magic link email arrives within 5 seconds (Resend SLA)
- ✅ Onboarding is 3 steps, ~2 minutes
- ✅ CSV import with column mapping takes ~1 minute
- ✅ AI categorization runs in background, user sees results within 30 seconds
- ⚠️ Risk: if CSV has unusual column names, mapping might confuse; mitigate with smart column-name inference

---

## Journey 2: Household Manager — "Monthly Review"

**Persona:** David, manages finances for a family of four  
**Goal:** Review last month's spending across 8 accounts, adjust budget  
**Duration:** ~15 minutes

```text
STEP 1: Sign In
  David clicks bookmarked Finima URL → auto-refreshes JWT → lands on Dashboard

STEP 2: Dashboard Review
  Net worth widget: $127,400 (↑ $2,100 from last month)
  Cash flow: Income $8,200, Expenses $6,450 → surplus $1,750
  Top spending: Housing $2,100, Food $980, Transportation $650
  Budget alerts: "Shopping exceeded budget by $120"

STEP 3: Drill into Shopping
  Clicks "Shopping" on donut chart → filtered transaction table
  Sees 23 transactions. Notices "AMZN*1234" categorized as "Other"
  Clicks category → changes to "Shopping > Online"
  Confirms "Apply to all Amazon transactions" → 7 more updated

STEP 4: Check Recurring
  Navigates to Recurring → sees newly detected: "Disney+ $13.99/mo"
  Confirms it. Dismisses a false positive ("one-time Costco purchase")

STEP 5: Update Budget
  Goes to Budget page → increases "Shopping" limit from $400 to $500
  Adds new category "Kids Activities" with $200/mo limit
  Clicks "Auto-Suggest" for remaining categories → reviews and accepts

STEP 6: Import New Statements
  Downloads this month's OFX from two banks
  Uploads both to their respective accounts
  OFX files import instantly (no column mapping needed)
  AI categorizes 240 transactions in ~45 seconds

STEP 7: Check Savings Goals
  "Emergency Fund" goal: $12,000 / $15,000 (80%)
  Projected completion: July 2026 at current savings rate
  David feels on track.
```

---

## Journey 3: Privacy Advocate — "Self-Hosted Setup"

**Persona:** Kenji, software engineer, runs everything on a home server  
**Goal:** Deploy Finima locally with full data sovereignty  
**Duration:** ~30 minutes for setup

```text
STEP 1: Clone & Configure
  git clone https://github.com/pacphi/finima.git
  cp .env.example .env
  # Edits .env with secrets: APP__AUTH__JWT_SECRET, APP__RESEND__API_KEY
  # Edits config/production.yaml for non-secret settings (CORS origins, log level)

STEP 2: Pull Model
  ollama pull gemma4:26b-a4b-it-q4_K_M
  # Model downloads ~15GB; Kenji has RTX 4090

STEP 3: Start Services
  make start  # Starts infra (PostgreSQL + MinIO + Ollama when LLM=ollama), then backend + frontend

STEP 4: Verify
  Opens http://localhost:5173
  Signs in with email → magic link arrives via Resend
  Creates portfolio, imports 3 years of OFX files from all accounts
  AI categorization runs locally; no data leaves his network

STEP 5: Ongoing Use
  Sets up a cron to backup PostgreSQL daily
  Accesses Finima from any device on home LAN
  Emotion: "My financial data is mine. Period."
```

---

## Journey 4: Financial Learner — "Understanding My Spending"

**Persona:** Mia, recent college graduate, first real job  
**Goal:** Understand where her money goes and create a budget  
**Duration:** ~20 minutes over first session

```text
STEP 1: Sign Up & Import
  Mia signs up, creates "Mia's Money" portfolio
  Adds checking account, downloads 3 months of CSV from her credit union
  Imports and AI categorizes everything

STEP 2: Dashboard Shock
  Dashboard shows: $2,800/mo income, $2,650/mo expenses
  Spending breakdown: Food $680 (!), Shopping $520, Subscriptions $85
  Mia: "I had no idea I spent that much on food..."

STEP 3: Explore Categories
  Clicks "Food & Dining" → sees individual transactions
  Discovers: 22 DoorDash orders ($14 avg), 8 Starbucks visits
  Insight card: "Your food spending is 24% of income. National avg is 12%."

STEP 4: Set First Budget
  Clicks "Auto-Suggest Budget" → system proposes limits based on 50/30/20 rule
  Needs: $1,400 (50%), Wants: $840 (30%), Savings: $560 (20%)
  Mia adjusts Food from $680 → $400 goal
  Sets up a Savings Goal: "Emergency Fund" — $2,000 target

STEP 5: Learn
  Browses News/Learn section
  Reads: "5 Steps to Build an Emergency Fund" (Investopedia)
  LLM summary: "Start with $500, automate transfers, aim for 3-6 months expenses."
  Relevance badge: ⭐⭐⭐ (matches her savings goal)

STEP 6: Monthly Check-In
  Next month, Mia re-imports transactions
  Budget page shows: Food $390 ✅ (under $400 target!)
  Dashboard: savings rate improved from 5% to 12%
  Emotion: "I can actually do this."
```

---

## Journey 5: Returning User — "Correcting AI Mistakes"

**Persona:** Any authenticated user  
**Goal:** Fix bulk misclassifications from a new import  
**Duration:** ~5 minutes

```text
STEP 1: Review Flagged Transactions
  After import, user sees notification: "3 transactions need review"
  Navigates to Transactions → filter: "Needs Review"

STEP 2: Individual Fix
  "CHECKCARD 0423 SQ *GREENLEAF" → AI said "Other" (confidence: 0.42)
  User changes to "Food & Dining > Restaurants"
  Confirms: "Apply to all SQ *GREENLEAF" → creates override rule

STEP 3: Bulk Fix
  Notices 12 transactions from "VENMO" all categorized as "Transfer"
  Some should be "Food" (dinner splits), some "Entertainment"
  Selects 5 dinner-related ones → Bulk Edit → "Food & Dining > Restaurants"
  Selects 4 entertainment ones → Bulk Edit → "Entertainment > Events"
  Leaves 3 as "Transfer" (they were actual transfers)

STEP 4: Verify
  Refreshes dashboard → spending breakdown now accurate
  Category overrides will improve future AI accuracy
```

---

## Journey Map Summary

| Journey           | Steps | Key Emotion                       | Critical Moment                  |
| ----------------- | ----- | --------------------------------- | -------------------------------- |
| First-Time User   | 8     | Satisfaction → "That was easy!"   | Column mapping UX clarity        |
| Household Manager | 7     | Control → "I'm on top of this"    | Cross-account aggregate accuracy |
| Self-Hosted Setup | 5     | Empowerment → "My data, my rules" | Docker Compose reliability       |
| Financial Learner | 6     | Surprise → Motivation             | Spending insight card impact     |
| Correcting AI     | 4     | Relief → Trust                    | Bulk edit efficiency             |

---

## Journey 6: Household Manager — "Where Does My Paycheck Go?"

**Persona:** David (same as Journey 2), manages 8 accounts  
**Goal:** Understand how his biweekly paycheck distributes across credit cards, loans, and savings  
**Duration:** ~10 minutes (first setup), ~3 minutes per monthly review

```text
STEP 1: Tag Primary Account
  David goes to Accounts → Chase Checking → ⋮ → Edit
  Toggles "Primary Income Account" ON
  His partner's account at Ally is also tagged as primary

STEP 2: System Detects Flows (automatic, background)
  System scans 3 months of transactions across all 8 accounts
  Matches: "TRANSFER TO SAVINGS -$500" in Chase ↔ "+$500" in Ally Savings
  Matches: "AUTOPAY AMEX -$650" in Chase ↔ "PAYMENT RECEIVED +$650" in Amex
  Matches: "MORTGAGE PMT -$1,800" in Chase → one-sided (mortgage not imported)
  Detects: "STUDENT LOAN -$280" in Chase → one-sided
  Total: 12 flow pairs detected, 4 one-sided flows

STEP 3: Review Detected Flows
  Dashboard shows new "Money Flow" widget with Sankey diagram
  David sees:

  Chase Checking ($4,100/mo income)
    ├──→ Mortgage      $1,800  (43.9%)
    ├──→ Amex Gold       $650  (15.9%)
    ├──→ Ally Savings    $500  (12.2%)
    ├──→ Student Loan    $280   (6.8%)
    ├──→ Car Insurance   $145   (3.5%)
    └──→ Discretionary   $725  (17.7%)

  David: "Wow, almost 44% goes straight to mortgage. And Amex is
  eating 16%... I didn't realize it was that much."

STEP 4: Drill Into Amex Outflow
  Clicks the Amex band in Sankey → sees 3 autopay transactions/month
  Plus a trend line: Amex outflow was $520 three months ago, now $650
  LLM insight: "Your Amex payments increased 25% over 3 months,
  driven by rising dining and travel spending."

STEP 5: Create Flow Group
  David groups: Mortgage + Property Tax + Insurance = "Housing Costs"
  Sankey updates: single "Housing Costs" band = $2,045/mo (49.9%)

STEP 6: Balance Impact Waterfall
  Switches to "Balance Impact" tab
  Sees waterfall: $3,200 start → +$4,100 income → -$1,800 mortgage
  → -$650 Amex → -$500 savings → -$280 loan → -$145 insurance
  → = $3,925 end balance
  David: "So I'm actually net positive $725/mo in checking. Good."

STEP 7: Monthly Review (subsequent months)
  Each month, David glances at the outflow ranking table
  Notices student loan dropped off (paid off!) → $280/mo freed up
  Redirects it: increases savings transfer to $780/mo
  Emotion: "I can see exactly where every dollar flows. Control."
```

**Key Moments:**

- ✅ Tagging primary account is a single toggle — no configuration fatigue
- ✅ Auto-detection catches most flows; manual linking handles edge cases
- ✅ Sankey makes the "invisible drain" of autopay obligations viscerally clear
- ✅ Trend arrows on outflow ranking catch creeping increases early
- ⚠️ Risk: one-sided flows (user hasn't imported all accounts) → clear messaging needed
