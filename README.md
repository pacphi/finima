# Finima

**Privacy-first, self-hosted personal finance intelligence.**

Your financial data never leaves your server. Finima combines a high-performance Rust backend with local AI to give you deep insight into your spending, savings, and cash flow -- without handing your data to a third party.

- **Fully self-hosted** -- runs on your hardware, your rules
- **Local AI categorization** -- Ollama-powered LLM classifies transactions on-device
- **Multi-format import** -- bring data from any bank via CSV, OFX, QIF, or XLSX
- **Built for speed** -- Rust backend, React frontend, PostgreSQL storage

**Stack:** Rust (Axum) | React 19 | TypeScript | PostgreSQL 16 | Ollama (Gemma 4) | Vite | Tailwind CSS

---

## Features

- **Multi-format file import** -- CSV, OFX, QIF, XLSX with column mapping UI
- **AI-powered categorization** -- local LLM via Ollama, no cloud API calls
- **Budget tracking** -- set budgets by category, track progress in real time
- **Savings goals** -- define targets, monitor contributions automatically
- **Recurring transaction detection** -- surfaces subscriptions and regular payments
- **Inter-account flow visualization** -- Sankey diagrams show money movement across accounts
- **Financial health score** -- composite gauge of spending, saving, and debt metrics
- **Cash flow and net worth charts** -- waterfall, donut, and time-series visualizations
- **Real-time progress** -- WebSocket updates during file imports and LLM processing
- **Passwordless auth** -- magic-link sign-in, no passwords to manage
- **News feed** -- aggregated financial articles with LLM summaries
- **Theming and preferences** -- dark/light mode, configurable currency and date formats

## Documentation — choose your path

| I want to…                               | Start here                                                                                                       |
| ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| **Use Finima** (no technical background) | [Getting Started Guide](docs/guides/getting-started.md) — step-by-step setup with no assumed technical knowledge |
| **Use Finima** (developer / technical)   | [Quick Start](docs/guides/quick-start.md) — concise setup for developers                                         |
| **Learn all the features**               | [User Guide](docs/guides/user-guide.md)                                                                          |
| **Look up a term**                       | [Glossary](docs/guides/glossary.md)                                                                              |
| **Contribute or run locally**            | [Maintainer Guide](docs/guides/maintainer-guide.md)                                                              |
| **Deploy to production**                 | [Deployment Guide](docs/guides/deployment.md)                                                                    |

## Quick Start

```bash
git clone https://github.com/cphillipson/finima.git
cd finima
cp .env.example .env
make start          # Start infrastructure + backend + frontend
# Open http://localhost:5173
```

Or step by step:

```bash
make docker-infra   # Start PostgreSQL + MinIO (and Ollama when LLM=ollama)
make dev            # Start backend (port 3000) + frontend (port 5173)
```

See [docs/guides/quick-start.md](docs/guides/quick-start.md) for the full setup guide.

## Documentation

| Guide                                                 | Description                     |
| ----------------------------------------------------- | ------------------------------- |
| [Quick Start](docs/guides/quick-start.md)             | Getting started                 |
| [Getting Started](docs/guides/getting-started.md)     | Non-technical first-run guide   |
| [Glossary](docs/guides/glossary.md)                   | Plain-language term definitions |
| [UI Overview](docs/guides/user-interface-overview.md) | Visual tour of the UI           |
| [User Guide](docs/guides/user-guide.md)               | End-user walkthrough            |
| [Maintainer Guide](docs/guides/maintainer-guide.md)   | Contributing and development    |
| [Deployment](docs/guides/deployment.md)               | Production deployment           |
| [Object Storage](docs/guides/object-storage-setup.md) | MinIO / S3 configuration        |
| [Backup & Recovery](docs/guides/database-backup.md)   | Database backup and restore     |
| [Observability](docs/guides/observability.md)         | Monitoring and dashboards       |
| [Troubleshooting](docs/guides/troubleshooting.md)     | Common issues and fixes         |
| [Architecture](docs/guides/architecture-overview.md)  | System design overview          |
| [ADRs](docs/ADRs/README.md)                           | Architecture Decision Records   |
| [DDDs](docs/DDDs/README.md)                           | Domain-Driven Design Documents  |

## Architecture

Finima is organized as a Rust workspace of 8 crates (`finima-core`, `finima-db`, `finima-api`, `finima-auth`, `finima-ingest`, `finima-llm`, `finima-analysis`, `finima-feed`) behind an Axum HTTP/WebSocket server. The React SPA communicates over REST and WebSocket. PostgreSQL is the single data store; Ollama runs the LLM locally; MinIO provides S3-compatible object storage for uploaded files.

See [docs/guides/architecture-overview.md](docs/guides/architecture-overview.md) for the full design, and [docs/ADRs/README.md](docs/ADRs/README.md) for the rationale behind key decisions.

## Testing

```bash
make test           # Fast: unit tests only (no Docker needed)
make test-all       # Full: auto-starts test DB, runs everything, stops DB
make test-llm       # LLM: auto-starts Ollama, pulls model, runs LLM tests
make ci             # CI: format + lint + typecheck + unit tests
```

See the [Maintainer Guide](docs/guides/maintainer-guide.md) for the full test reference.

## Contributing

Contributions are welcome. Read the [Maintainer Guide](docs/guides/maintainer-guide.md) for development setup, coding standards, and the PR workflow. Run `make ci` before submitting to verify formatting, linting, and tests pass.

## License

[MIT](LICENSE)
