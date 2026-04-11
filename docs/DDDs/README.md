# Domain-Driven Design Documents

| DDD                                         | Bounded Context                | Primary Crate(s)                |
| ------------------------------------------- | ------------------------------ | ------------------------------- |
| [DDD-001](DDD-001-identity-access.md)       | Identity & Access              | `finima-auth`                   |
| [DDD-002](DDD-002-portfolio-management.md)  | Portfolio Management           | `finima-core`, `finima-db`      |
| [DDD-003](DDD-003-transaction-ingestion.md) | Transaction Ingestion          | `finima-ingest`                 |
| [DDD-004](DDD-004-intelligence.md)          | Intelligence (LLM + Recurring) | `finima-llm`, `finima-analysis` |
| [DDD-005](DDD-005-financial-analysis.md)    | Financial Analysis             | `finima-analysis`               |
| [DDD-006](DDD-006-content-aggregation.md)   | Content Aggregation            | `finima-feed`                   |

## Context Map

```text
┌─────────────────┐     ┌─────────────────────┐
│ Identity &      │────▶│ Portfolio            │
│ Access          │     │ Management           │
│ (DDD-001)       │     │ (DDD-002)            │
└─────────────────┘     └──────────┬───────────┘
                                   │
                        ┌──────────┼───────────┐
                        │          │           │
                        ▼          ▼           ▼
              ┌──────────────┐ ┌───────────┐ ┌──────────────┐
              │ Transaction  │ │Intelligence│ │ Financial    │
              │ Ingestion    │─▶│ (DDD-004) │─▶│ Analysis     │
              │ (DDD-003)    │ └───────────┘ │ (DDD-005)    │
              └──────────────┘       │       └──────────────┘
                                     │
                                     ▼
                              ┌──────────────┐
                              │ Content      │
                              │ Aggregation  │
                              │ (DDD-006)    │
                              └──────────────┘
```

**Data flow:** Ingestion produces raw transactions → Intelligence categorizes and detects recurring patterns → Analysis computes dashboards, budgets, flows, and health scores. Content Aggregation uses the LLM client from Intelligence for article summarization.
