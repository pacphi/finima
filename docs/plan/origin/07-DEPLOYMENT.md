# Finima — Deployment Plan

**Version:** 1.0 | **Date:** 2026-04-10

---

## 1. Deployment Targets

| Target                    | Purpose                            | Tooling                             |
| ------------------------- | ---------------------------------- | ----------------------------------- |
| **Local Dev**             | Developer workstation              | `make dev` (cargo watch + vite dev) |
| **Docker Compose (dev)**  | Full-stack local with all services | `make docker-up`                    |
| **Docker Compose (prod)** | Self-hosted production             | `make docker-prod`                  |
| **CI/CD**                 | Automated testing + image build    | GitHub Actions                      |

---

## 2. Docker Compose — Development Profile

```yaml
# docker-compose.yml
version: '3.9'

services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: finima
      POSTGRES_PASSWORD: finima_dev
      POSTGRES_DB: finima
    ports:
      - '5432:5432'
    volumes:
      - pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ['CMD-SHELL', 'pg_isready -U finima']
      interval: 5s
      timeout: 3s
      retries: 5

  ollama:
    image: ollama/ollama:latest
    ports:
      - '11434:11434'
    volumes:
      - ollama_models:/root/.ollama
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: all
              capabilities: [gpu]
    # On first run: docker exec ollama ollama pull gemma4:26b-a4b-it-q4_K_M

  backend:
    build:
      context: .
      dockerfile: Dockerfile.backend
    environment:
      APP_ENV: development
      APP__DATABASE__URL: postgres://finima:finima_dev@postgres:5432/finima
      APP__LLM__OLLAMA__URL: http://ollama:11434
      APP__RESEND__API_KEY: ${RESEND_API_KEY:-}
      APP__AUTH__JWT_SECRET: ${JWT_SECRET:-dev-secret-change-me}
      APP__CORS__ALLOWED_ORIGINS: '["http://localhost:5173"]'
    ports:
      - '3000:3000'
    volumes:
      - ./config:/app/config:ro
    depends_on:
      postgres:
        condition: service_healthy
    command: ['./finima-api']

  frontend:
    build:
      context: ./frontend
      dockerfile: Dockerfile.frontend
      target: dev
    ports:
      - '5173:5173'
    volumes:
      - ./frontend/src:/app/src
    command: ['npm', 'run', 'dev', '--', '--host']

volumes:
  pgdata:
  ollama_models:
```

---

## 3. Docker Compose — Production Profile

```yaml
# docker-compose.prod.yml
version: '3.9'

services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: ${POSTGRES_USER}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: ${POSTGRES_DB:-finima}
    volumes:
      - pgdata_prod:/var/lib/postgresql/data
    healthcheck:
      test: ['CMD-SHELL', 'pg_isready -U ${POSTGRES_USER}']
      interval: 10s
      timeout: 5s
      retries: 5
    restart: unless-stopped

  ollama:
    image: ollama/ollama:latest
    volumes:
      - ollama_models_prod:/root/.ollama
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: all
              capabilities: [gpu]
    restart: unless-stopped

  backend:
    image: ghcr.io/pacphi/finima-backend:latest
    environment:
      APP_ENV: production
      APP__DATABASE__URL: postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/${POSTGRES_DB:-finima}
      APP__LLM__OLLAMA__URL: http://ollama:11434
      APP__RESEND__API_KEY: ${RESEND_API_KEY}
      APP__AUTH__JWT_SECRET: ${JWT_SECRET}
    volumes:
      - ./config:/app/config:ro
    depends_on:
      postgres:
        condition: service_healthy
    restart: unless-stopped

  frontend:
    image: ghcr.io/pacphi/finima-frontend:latest
    ports:
      - '80:80'
      - '443:443'
    volumes:
      - ./config/frontend-prod.yaml:/usr/share/nginx/html/config.yaml:ro
    restart: unless-stopped

  caddy:
    image: caddy:2-alpine
    ports:
      - '80:80'
      - '443:443'
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile
      - caddy_data:/data
    depends_on:
      - backend
      - frontend
    restart: unless-stopped

volumes:
  pgdata_prod:
  ollama_models_prod:
  caddy_data:
```

---

## 4. Dockerfiles

### Backend

```dockerfile
# Dockerfile.backend
FROM rust:1.82-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --release -p finima-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/finima-api /usr/local/bin/
EXPOSE 3000
CMD ["finima-api"]
```

### Frontend

```dockerfile
# frontend/Dockerfile.frontend

# Dev stage
FROM node:24-alpine AS dev
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
EXPOSE 5173

# Build stage
FROM dev AS build
RUN npm run build

# Prod stage
FROM nginx:alpine AS prod
COPY --from=build /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
```

---

## 5. Makefiles

### Root Makefile (Backend)

```makefile
.PHONY: dev build test lint clean docker-up docker-down docker-prod migrate

# ── Development ──────────────────────────────────
dev:
  cargo watch -x 'run -p finima-api'

build:
  cargo build --release -p finima-api

# ── Database ─────────────────────────────────────
migrate:
  sqlx migrate run --source crates/finima-db/src/migrations

migrate-create:
  sqlx migrate add -r $(name) --source crates/finima-db/src/migrations

# ── Testing ──────────────────────────────────────
test: test-unit test-integration

test-unit:
  cargo test --workspace --lib

test-integration:
  docker compose -f docker-compose.test.yml up -d postgres
  sleep 3
  DATABASE_URL="postgres://finima:test@localhost:5433/finima_test" \
    cargo test --workspace --test '*'
  docker compose -f docker-compose.test.yml down

test-crate:
  cargo test -p $(crate)

# ── Quality ──────────────────────────────────────
lint:
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings

fmt:
  cargo fmt --all

coverage:
  cargo llvm-cov --workspace --html --open

# ── Docker ───────────────────────────────────────
docker-up:
  docker compose up -d --build

docker-down:
  docker compose down

docker-prod:
  docker compose -f docker-compose.prod.yml up -d

docker-logs:
  docker compose logs -f backend

# ── Clean ────────────────────────────────────────
clean:
  cargo clean
  docker compose down -v
```

### Frontend Makefile

```makefile
.PHONY: dev build test test-e2e lint clean

# ── Development ──────────────────────────────────
dev:
  npm run dev

build:
  npm run build

preview:
  npm run preview

# ── Testing ──────────────────────────────────────
test:
  npm run test

test-watch:
  npm run test -- --watch

test-e2e:
  npx playwright test

test-e2e-ui:
  npx playwright test --ui

test-e2e-headed:
  npx playwright test --headed

# ── Quality ──────────────────────────────────────
lint:
  npm run lint
  npx tsc --noEmit

fmt:
  npx prettier --write src/

# ── Dependencies ─────────────────────────────────
install:
  npm ci

update:
  npm update

# ── Clean ────────────────────────────────────────
clean:
  rm -rf dist node_modules .vite
```

---

## 6. GitHub Actions Workflows

### CI Workflow (`.github/workflows/ci.yml`)

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  backend-lint:
    name: Backend Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets -- -D warnings

  backend-test:
    name: Backend Tests
    runs-on: ubuntu-latest
    needs: backend-lint
    services:
      postgres:
        image: postgres:16-alpine
        env:
          POSTGRES_USER: finima
          POSTGRES_PASSWORD: test
          POSTGRES_DB: finima_test
        ports:
          - 5432:5432
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    env:
      DATABASE_URL: postgres://finima:test@localhost:5432/finima_test
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Run unit tests
        run: cargo test --workspace --lib
      - name: Run integration tests
        run: cargo test --workspace --test '*'

  frontend-lint:
    name: Frontend Lint
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: frontend
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 24
          cache: npm
          cache-dependency-path: frontend/package-lock.json
      - run: npm ci
      - run: npm run lint
      - run: npx tsc --noEmit

  frontend-test:
    name: Frontend Tests
    runs-on: ubuntu-latest
    needs: frontend-lint
    defaults:
      run:
        working-directory: frontend
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 24
          cache: npm
          cache-dependency-path: frontend/package-lock.json
      - run: npm ci
      - run: npm run test -- --run

  e2e-test:
    name: E2E Tests
    runs-on: ubuntu-latest
    needs: [backend-test, frontend-test]
    services:
      postgres:
        image: postgres:16-alpine
        env:
          POSTGRES_USER: finima
          POSTGRES_PASSWORD: test
          POSTGRES_DB: finima_test
        ports:
          - 5432:5432
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    env:
      DATABASE_URL: postgres://finima:test@localhost:5432/finima_test
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - uses: actions/setup-node@v4
        with:
          node-version: 24
          cache: npm
          cache-dependency-path: frontend/package-lock.json
      - name: Build backend
        run: cargo build --release -p finima-api
      - name: Start backend
        run: |
          JWT_SECRET=test-secret \
          RESEND_API_KEY=re_test \
          OLLAMA_URL=http://localhost:11434 \
          FRONTEND_URL=http://localhost:5173 \
          ./target/release/finima-api &
        env:
          DATABASE_URL: postgres://finima:test@localhost:5432/finima_test
      - name: Install frontend deps
        working-directory: frontend
        run: npm ci
      - name: Install Playwright browsers
        working-directory: frontend
        run: npx playwright install --with-deps chromium
      - name: Build frontend
        working-directory: frontend
        run: npm run build
      - name: Run E2E tests
        working-directory: frontend
        run: npx playwright test
      - uses: actions/upload-artifact@v4
        if: failure()
        with:
          name: playwright-report
          path: frontend/playwright-report/
```

### Release Workflow (`.github/workflows/release.yml`)

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: read
  packages: write

jobs:
  build-and-push:
    name: Build & Push Docker Images
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Login to GHCR
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract version
        id: version
        run: echo "VERSION=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT

      - name: Build and push backend
        uses: docker/build-push-action@v5
        with:
          context: .
          file: Dockerfile.backend
          push: true
          tags: |
            ghcr.io/pacphi/finima-backend:latest
            ghcr.io/pacphi/finima-backend:${{ steps.version.outputs.VERSION }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

      - name: Build and push frontend
        uses: docker/build-push-action@v5
        with:
          context: ./frontend
          file: frontend/Dockerfile.frontend
          target: prod
          push: true
          tags: |
            ghcr.io/pacphi/finima-frontend:latest
            ghcr.io/pacphi/finima-frontend:${{ steps.version.outputs.VERSION }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          generate_release_notes: true
```

---

## 7. Configuration

All configuration is externalized into **YAML files** with a layered override hierarchy. Secrets are injected via environment variables (never committed to version control). See [ADR-009](../ADRs/ADR-009-externalized-yaml-configuration.md) for full rationale.

### Backend Configuration (config-rs + YAML)

```yaml
# config/default.yaml — Base configuration (all keys, safe defaults)
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
  provider: 'ollama'
  ollama:
    url: 'http://localhost:11434'
    model: 'gemma4:26b-a4b-it-q4_K_M'
  llamacpp:
    model_path: ''

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
  format: 'pretty'

cors:
  allowed_origins:
    - 'http://localhost:5173'
```

```yaml
# config/production.yaml — Prod overrides (secrets injected via env vars)
database:
  max_connections: 25

logging:
  level: 'info'
  format: 'json'

cors:
  allowed_origins: [] # Set via APP__CORS__ALLOWED_ORIGINS env var
```

```yaml
# config/test.yaml — Test profile (test DB, no real email, no real LLM)
database:
  url: 'postgres://finima:test@localhost:5433/finima_test'

auth:
  jwt_secret: 'test-secret-not-for-production'

resend:
  api_key: 're_test_mock'

llm:
  provider: 'ollama'
  ollama:
    url: 'http://localhost:11434'
```

### Frontend Configuration (runtime YAML)

```yaml
# frontend/public/config.yaml — Fetched at app startup, not baked into build
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

For Docker production, mount a per-environment frontend config:

```yaml
# In docker-compose.prod.yml
frontend:
  volumes:
    - ./config/frontend-prod.yaml:/usr/share/nginx/html/config.yaml:ro
```

### Secrets (.env — gitignored, secrets only)

```bash
# .env.example — Only secrets, never structural config
APP__AUTH__JWT_SECRET=change-me-to-a-64-char-random-string
APP__RESEND__API_KEY=re_xxxxxxxxxxxx
APP__DATABASE__URL=postgres://finima:STRONG_PASSWORD@postgres:5432/finima
POSTGRES_PASSWORD=STRONG_PASSWORD
```

**Loading order:** `config/default.yaml` → `config/{APP_ENV}.yaml` → environment variables (double-underscore separator: `APP__DATABASE__URL` overrides `database.url`).

---

## 8. Deployment Checklist

### Self-Hosted Production Deployment

1. **Prerequisites:**
   - Docker Engine 24+ with Compose v2
   - NVIDIA GPU + drivers (for Ollama GPU acceleration; CPU fallback works but slower)
   - Domain name with DNS A record pointing to server IP
   - Resend account with verified domain

2. **Steps:**

   ```bash
   git clone https://github.com/pacphi/finima.git
   cd finima
   cp .env.example .env
   # Edit .env with secrets only:
   #   - APP__AUTH__JWT_SECRET (openssl rand -hex 32)
   #   - APP__RESEND__API_KEY
   #   - POSTGRES_PASSWORD
   # Edit config/production.yaml for non-secret settings:
   #   - cors.allowed_origins: ["https://finima.yourdomain.com"]
   # Edit config/frontend-prod.yaml for frontend:
   #   - api.base_url: "https://finima.yourdomain.com/api"
   #   - api.ws_url: "wss://finima.yourdomain.com/api"

   # Pull and start
   make docker-prod

   # Pull Gemma 4 model (first time only)
   docker exec finima-ollama-1 ollama pull gemma4:26b-a4b-it-q4_K_M

   # Verify
   curl http://localhost:3000/health
   ```

3. **Post-deployment:**
   - Configure Caddy with your domain for automatic HTTPS
   - Set up PostgreSQL backup cron (`pg_dump` daily to encrypted storage)
   - Monitor logs: `docker compose -f docker-compose.prod.yml logs -f`
   - Set up uptime monitoring (UptimeRobot, Healthchecks.io)

---

## 9. Rollback Strategy

- Docker images are tagged by version (`v1.0.0`, `v1.0.1`, etc.)
- Rollback: `docker compose -f docker-compose.prod.yml pull backend=ghcr.io/pacphi/finima-backend:v1.0.0 && docker compose -f docker-compose.prod.yml up -d`
- Database migrations include down migrations for reversibility
- Rollback migration: `sqlx migrate revert --source crates/finima-db/src/migrations`
