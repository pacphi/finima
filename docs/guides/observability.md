# Observability with SigNoz

Finima includes a self-hosted SigNoz observability stack for metrics, traces, and logs, powered by OpenTelemetry.

## Enabling SigNoz

Start the observability stack alongside the main services:

```bash
make observability
```

Or manually:

```bash
docker compose -f docker-compose.yml -f docker-compose.observability.yml up -d
```

The SigNoz UI is available at <http://localhost:3301>.

## Architecture

The observability stack consists of:

- **OpenTelemetry Collector** -- Scrapes Prometheus metrics from the backend `/metrics` endpoint every 15 seconds, collects Docker container logs, and receives OTLP traces/metrics/logs from instrumented services.
- **SigNoz** -- The UI and query engine for exploring metrics, traces, and logs.
- **ClickHouse** -- Time-series database backend used by SigNoz for storage.

```text
Backend (/metrics) --> OTel Collector --> SigNoz --> ClickHouse
Docker logs --------->
OTLP (traces) ------->
```

## Dashboards

Three pre-configured dashboards are included in `config/signoz/dashboards/`:

### Security & Compliance

**File:** `security-compliance.json`

For security teams and compliance audits:

- **Magic Links Sent vs Verified** -- Tracks authentication flow completion. A large gap between sent and verified may indicate phishing or brute-force.
- **Auth Failure Rate** -- Gauge showing the proportion of unverified magic links.
- **Session Revocations** -- Explicit logout or forced session invalidation events.
- **Rate Limit Hits** -- Requests rejected by rate limiting. Spikes indicate potential abuse.
- **4xx/5xx by Endpoint** -- Client and server errors broken down by API path.
- **CORS Violations** -- Requests blocked by CORS policy.

### Operations

**File:** `operations.json`

For SRE and operations teams:

- **Request Latency (p50, p95, p99)** -- Response time percentiles across all endpoints.
- **Error Rate** -- Ratio of 5xx responses to total requests.
- **Error Budget Burn Rate** -- How fast the 99.9% availability SLO budget is being consumed. Values above 1.0 are unsustainable.
- **DB Connection Pool Utilization** -- Active vs maximum database connections.
- **Active WebSocket Connections** -- Current real-time connection count.
- **Upload Success/Failure Rate** -- File upload outcomes over time.
- **Container Memory/CPU** -- Resource consumption of Finima containers.

### Developer

**File:** `developer.json`

For developers debugging performance and feature behavior:

- **LLM Categorization Duration** -- Time for the LLM to categorize transactions (p95).
- **LLM Categorization Success Rate** -- Proportion of successful categorization calls.
- **Transaction Import Throughput** -- Rate of transactions ingested from CSV uploads or bank feeds.
- **API Response Times by Path** -- Per-endpoint latency breakdown.
- **DB Query Latency Distribution** -- Heatmap of database query durations.
- **Feed Fetch Duration** -- Time to pull and parse RSS/Atom feed sources.

## Setting Up Alerts

SigNoz supports alert rules via its UI. Recommended alerts for Finima:

### Critical

| Alert              | Condition                            | Channel           |
| ------------------ | ------------------------------------ | ----------------- |
| High error rate    | 5xx rate > 5% for 5 minutes          | PagerDuty / Slack |
| Error budget burn  | Burn rate > 10x for 5 minutes        | PagerDuty / Slack |
| DB pool exhaustion | Pool utilization > 95% for 2 minutes | PagerDuty / Slack |

### Warning

| Alert              | Condition                                        | Channel |
| ------------------ | ------------------------------------------------ | ------- |
| Elevated latency   | p99 > 2s for 10 minutes                          | Slack   |
| Rate limit surge   | Rate limit hits > 100/min for 5 minutes          | Slack   |
| Auth failure spike | Auth failure rate > 60% for 15 minutes           | Slack   |
| LLM degradation    | Categorization success rate < 85% for 10 minutes | Slack   |

### Informational

| Alert               | Condition                            | Channel |
| ------------------- | ------------------------------------ | ------- |
| Feed fetch failures | Any feed fails 3 consecutive fetches | Slack   |
| High memory usage   | Container memory > 80% limit         | Slack   |

To create alerts in SigNoz:

1. Open the SigNoz UI at <http://localhost:3301>.
2. Navigate to **Alerts** in the sidebar.
3. Click **New Alert Rule**.
4. Select the metric, set the threshold and evaluation window.
5. Configure the notification channel (Slack webhook, PagerDuty, email, etc.).

## Configuration

The OpenTelemetry Collector config is at `config/otel-collector-config.yaml`. Key settings:

- **Scrape interval:** 15 seconds (adjustable in the `prometheus` receiver section).
- **Batch size:** 10,000 data points with a 10-second timeout.
- **Memory limiter:** Caps at 80% of available memory to prevent OOM.

## Disabling Observability

To run without the observability stack, simply omit the observability compose file:

```bash
docker compose -f docker-compose.yml up -d
```

The backend still exposes `/metrics` for external scraping if needed.
