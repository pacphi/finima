# ADR-009: Externalized YAML Configuration

**Status:** Accepted  
**Date:** 2026-04-10  
**Deciders:** Chris Phillipson

---

## Context

Finima requires configuration for database connections, LLM endpoints, email API keys, JWT secrets, server binding, frontend API URLs, and feature toggles. The initial design used `dotenvy` (`.env` files) for backend configuration and Vite `VITE_*` environment variables for the frontend. This approach has several drawbacks:

1. **Flat key-value structure** — `.env` files cannot represent nested configuration (e.g., LLM provider settings, rate limit tuning, feed source lists) without naming conventions (`LLM_OLLAMA_URL`, `LLM_OLLAMA_MODEL`, etc.).
2. **No schema or validation** — `.env` values are all strings; type errors surface at runtime.
3. **No environment layering** — overriding a subset of values for dev/test/prod requires separate `.env` files with full duplication or manual merging.
4. **Frontend config scattered** — Vite `VITE_*` variables are baked into the build artifact at compile time, making runtime reconfiguration impossible without a rebuild.

## Decision

**All configuration for both backend and frontend is externalized into YAML files** with a layered override hierarchy.

### Backend Configuration

Use the **`config-rs`** crate (with YAML backend) to load configuration from a layered hierarchy:

```text
config/
├── default.yaml          # Base configuration (all keys, safe defaults)
├── development.yaml      # Dev overrides (local URLs, debug logging)
├── test.yaml             # Test overrides (test DB, mock endpoints)
└── production.yaml       # Prod overrides (real secrets, prod URLs)
```

**Loading order** (later overrides earlier):

1. `config/default.yaml` — always loaded.
2. `config/{APP_ENV}.yaml` — loaded based on `APP_ENV` environment variable (`development`, `test`, `production`).
3. Environment variables — `APP__DATABASE__URL` overrides `database.url` in YAML (double-underscore separator).

This gives a clean hierarchy: YAML files define structure, `APP_ENV` selects the profile, and env vars provide secret injection for production (e.g., CI/CD secrets, Docker Compose `environment:` blocks).

**Backend YAML structure:**

```yaml
# config/default.yaml
server:
  host: '0.0.0.0'
  port: 3000

database:
  url: 'postgres://finima:finima_dev@localhost:5432/finima'
  max_connections: 10

auth:
  jwt_secret: 'change-me-in-production'
  magic_link_expiry_minutes: 15
  from_email: 'auth@finima.app'
  rate_limit_per_hour: 5

resend:
  api_key: ''

llm:
  provider: 'candle' # "candle" | "ollama" | "stub"
  model: 'auto' # "auto" | explicit model name
  candle:
    model_id: 'google/gemma-4-E4B-it'
    model_path: ''
    quantization: 'Q4_K_M'
    device: 'auto'
    context_length: 8192
    threads: 0
  ollama:
    url: 'http://localhost:11434'
    model: 'gemma4:26b-a4b-it-q4_K_M'

feed:
  poll_interval_hours: 6
  sources:
    - name: 'Investopedia'
      url: 'https://www.investopedia.com/feedbuilder/feed/getfeed/?feedName=rss_articles'
      topic: 'investing'
    - name: 'NerdWallet'
      url: 'https://www.nerdwallet.com/blog/feed/'
      topic: 'budgeting'

logging:
  level: 'debug'
  format: 'pretty' # "pretty" | "json"

cors:
  allowed_origins:
    - 'http://localhost:5173'
```

```yaml
# config/production.yaml
database:
  max_connections: 25

logging:
  level: 'info'
  format: 'json'

cors:
  allowed_origins: [] # Set via APP__CORS__ALLOWED_ORIGINS env var
```

**Rust config struct (serde):**

```rust
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub resend: ResendConfig,
    pub llm: LlmConfig,
    pub feed: FeedConfig,
    pub logging: LoggingConfig,
    pub cors: CorsConfig,
}
```

### Frontend Configuration

The frontend uses a **runtime configuration file** served as a static asset, not baked into the build:

```text
frontend/public/config.yaml
```

At app startup, the React app fetches `/config.yaml`, parses it (using `js-yaml`), and hydrates a Zustand `configStore`. This enables runtime reconfiguration without rebuilding the frontend.

**Frontend YAML structure:**

```yaml
# frontend/public/config.yaml
api:
  base_url: 'http://localhost:3000'
  ws_url: 'ws://localhost:3000'

features:
  news_feed: true
  flow_analysis: true

defaults:
  currency: 'USD'
  date_format: 'MM/DD/YYYY'
  theme: 'system'
```

For Docker deployments, `config.yaml` is mounted as a volume, allowing per-environment customization without image rebuilds:

```yaml
# docker-compose.prod.yml
frontend:
  volumes:
    - ./config/frontend-prod.yaml:/usr/share/nginx/html/config.yaml:ro
```

### Secrets Handling

Secrets (JWT secret, Resend API key, database password) are **never stored in YAML files committed to version control**. They are injected via:

1. Environment variables (override YAML values): `APP__AUTH__JWT_SECRET`, `APP__RESEND__API_KEY`, `APP__DATABASE__URL`.
2. Docker Compose `environment:` or `env_file:` referencing a `.env` file that is `.gitignore`d.
3. CI/CD secret injection (GitHub Actions secrets).

The `.env` file still exists but **only for secret injection**, not for structural configuration. All non-secret configuration lives in YAML.

## Consequences

**Positive:**

- Nested, structured configuration with clear hierarchy and grouping.
- Type-safe deserialization via serde in Rust — misconfigurations caught at startup, not at first use.
- Environment layering without duplication — `production.yaml` only overrides what differs from `default.yaml`.
- Frontend config is runtime-swappable — no rebuild needed to change API URLs or toggle features.
- Feed sources, rate limits, and CORS origins are configurable without code changes.
- YAML is human-readable and supports comments (unlike `.env`).

**Negative:**

- Adds `config-rs` and `serde_yaml` dependencies to the backend.
- Adds `js-yaml` dependency to the frontend.
- Developers must understand the layering order (YAML < env vars).
- Frontend config fetch adds a small delay at app startup (~10-50ms). Mitigated: show a loading spinner during config fetch.

## Alternatives Considered

1. **`.env` files only (dotenvy)** — Flat, unstructured, no validation, no layering. Rejected.
2. **TOML configuration** — Rust-native but less widely known outside Rust ecosystem. YAML is more universal (Docker, Kubernetes, GitHub Actions all use YAML). Rejected.
3. **JSON configuration** — No comment support, verbose for nested structures. Rejected.
4. **Build-time env vars for frontend (Vite `VITE_*`)** — Requires rebuild for every config change. Rejected for production; acceptable as a development fallback.
