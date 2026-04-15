# ============================================================================
# Finima — Root Makefile
# ============================================================================
# Full-stack financial intelligence platform.
# Rust backend + React/Vite frontend.
#
# Quick Start:
#   make help              - Show all available targets
#   make install           - Install all dependencies
#   make start             - Start everything (infra + backend + frontend)
#   make dev               - Start backend + frontend (assumes infra running)
#   make docker-infra      - Start dev infrastructure
#   make ci                - Run full CI pipeline
# ============================================================================

# ============================================================================
# Variables and Configuration
# ============================================================================

SHELL := /bin/bash
.DEFAULT_GOAL := help

# Load .env if present so Docker Compose targets and Make-level variables
# (e.g. POSTGRES_PASSWORD for docker-compose.yml substitution) are available.
# The backend also loads .env itself via dotenvy, so APP__* vars work
# regardless of whether the user starts via Make or cargo run directly.
# Values must be bare (unquoted) — both Make and Docker Compose read literally.
ifneq (,$(wildcard ./.env))
    include .env
    export
endif

BACKEND_DIR  := .
FRONTEND_DIR := frontend

COMPOSE      := docker compose

# Auto-detect hardware for GPU acceleration
HAS_NVIDIA   := $(shell command -v nvidia-smi >/dev/null 2>&1 && nvidia-smi >/dev/null 2>&1 && echo 1 || echo 0)
HAS_METAL    := $(shell xcrun -find metal >/dev/null 2>&1 && echo 1 || echo 0)
GPU_OVERLAY  := $(if $(filter 1,$(HAS_NVIDIA)), -f docker-compose.gpu.yml,)

COMPOSE_DEV  := $(COMPOSE) -f docker-compose.yml$(GPU_OVERLAY)
COMPOSE_PROD := $(COMPOSE) -f docker-compose.prod.yml$(GPU_OVERLAY)
COMPOSE_TEST := $(COMPOSE) -f docker-compose.test.yml
COMPOSE_OBS  := $(COMPOSE) -f docker-compose.yml -f docker-compose.observability.yml$(GPU_OVERLAY)

# ── LLM backend selection ──────────────────────────────────────
# Set LLM= to control which AI backend is compiled and which Docker
# services are started.  Valid values:
#   candle        – in-process inference (CPU)
#   candle-metal  – in-process inference (Apple Metal GPU)
#   candle-cuda   – in-process inference (NVIDIA CUDA GPU)
#   ollama        – HTTP inference via Ollama container
#   none          – no LLM; categorization uses Tiers 0-2 only
#
# Default: none (no LLM — categorization uses Tiers 0-2 only).
# Override with: make start LLM=ollama  or  make start LLM=candle
LLM ?= none

# Auto-promote bare "candle" to the best accelerator for this machine.
ifeq ($(LLM),candle)
  ifeq ($(HAS_NVIDIA),1)
    override LLM := candle-cuda
  else ifeq ($(HAS_METAL),1)
    override LLM := candle-metal
  endif
endif

# Derive Cargo feature flags from LLM choice.
ifeq ($(LLM),ollama)
  CARGO_LLM_FEATURES := --features ollama
else ifeq ($(LLM),candle-metal)
  CARGO_LLM_FEATURES := --features candle,metal
else ifeq ($(LLM),candle-cuda)
  CARGO_LLM_FEATURES := --features candle,cuda
else ifeq ($(LLM),candle)
  CARGO_LLM_FEATURES := --features candle
else
  CARGO_LLM_FEATURES :=
endif

# ── Ollama detection ──────────────────────────────────────────
# When LLM=ollama, detect whether a local Ollama is already serving
# on port 11434 so we can skip the Docker container.
OLLAMA_PORT     ?= 11434
OLLAMA_MODEL    ?= gemma4:26b-a4b-it-q4_K_M
OLLAMA_LOCAL    := $(shell curl -sf http://localhost:$(OLLAMA_PORT)/api/version >/dev/null 2>&1 && echo 1 || echo 0)
OLLAMA_DOCKER   := $(shell docker inspect -f '{{.State.Running}}' finima-ollama 2>/dev/null)

# Determine infrastructure services to start.
# - LLM != ollama  → postgres + minio only (no ollama needed)
# - LLM  = ollama  → postgres + minio + ollama UNLESS a local Ollama is
#                     already responding (avoids port conflicts)
ifeq ($(LLM),ollama)
  ifeq ($(OLLAMA_LOCAL),1)
    INFRA_SERVICES := postgres minio
    OLLAMA_SOURCE  := local
  else
    INFRA_SERVICES := postgres minio ollama
    OLLAMA_SOURCE  := docker
  endif
else
  INFRA_SERVICES := postgres minio
  OLLAMA_SOURCE  := none
endif

LYCHEE := $(shell command -v lychee 2>/dev/null)

PRUNE_DIRS := \( -name node_modules -o -name target -o -name .claude \
	-o -name .claude-flow -o -name .git -o -name .swarm \
	-o -name dist -o -name coverage \) -prune

# Colors
BOLD   := $(shell tput bold 2>/dev/null || echo '')
GREEN  := $(shell tput setaf 2 2>/dev/null || echo '')
YELLOW := $(shell tput setaf 3 2>/dev/null || echo '')
BLUE   := $(shell tput setaf 4 2>/dev/null || echo '')
CYAN   := $(shell tput setaf 6 2>/dev/null || echo '')
RESET  := $(shell tput sgr0 2>/dev/null || echo '')

# ============================================================================
# Default Target
# ============================================================================

.PHONY: help
help:
	@echo "$(BOLD)$(BLUE)╔════════════════════════════════════════════════════════════════════╗$(RESET)"
	@echo "$(BOLD)$(BLUE)║                        Finima Makefile                             ║$(RESET)"
	@echo "$(BOLD)$(BLUE)╚════════════════════════════════════════════════════════════════════╝$(RESET)"
	@echo ""
	@echo "$(BOLD)Quick Start:$(RESET)"
	@echo "  make install           - Install all dependencies"
	@echo "  make start             - Start everything (infra + backend + frontend)"
	@echo "  make dev               - Start backend + frontend (assumes infra running)"
	@echo "  make docker-infra      - Start dev infrastructure"
	@echo "  make ci                - Run full CI pipeline"
	@echo "  make test              - Run all tests"
	@echo ""
	@echo "$(BOLD)LLM Backend (current: $(LLM)$(if $(filter ollama,$(LLM)), — $(OLLAMA_SOURCE),)):$(RESET)"
	@echo "  LLM=candle  make dev   - In-process inference (auto-detects Metal/CUDA/CPU)"
	@echo "  LLM=ollama  make dev   - HTTP inference (auto-detects local vs Docker)"
	@echo "  LLM=none    make dev   - No LLM (Tiers 0-2 only)"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Install & Build ═════════════════════════════════════════════════$(RESET)"
	@echo "  install                - Install all dependencies (backend + frontend)"
	@echo "  build                  - Build all (backend debug + frontend)"
	@echo "  build-release          - Build backend in release mode"
	@echo "  start                  - Start everything (infra + backend + frontend)"
	@echo "  dev                    - Start backend + frontend (assumes infra running)"
	@echo "  dev-backend            - Start backend API server only"
	@echo "  dev-watch              - Start backend with auto-reload (cargo-watch)"
	@echo "  clean                  - Clean build artifacts"
	@echo "  clean-all              - Clean build + Docker volumes (DESTROYS DATA)"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Test ════════════════════════════════════════════════════════════$(RESET)"
	@echo "  test                   - Run unit tests only (backend + frontend)"
	@echo "  test-all               - Run ALL tests (auto-starts/stops test DB)"
	@echo "  test-unit              - Run backend unit tests (no DB needed)"
	@echo "  test-integration       - Run backend integration tests (auto-starts DB)"
	@echo "  test-llm               - Run LLM tests (auto-starts Ollama, pulls model)"
	@echo "  test-frontend          - Run frontend unit tests"
	@echo "  test-e2e               - Run end-to-end tests (requires running backend)"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Lint & Format ═══════════════════════════════════════════════════$(RESET)"
	@echo "  lint                   - Lint everything (code + docs)"
	@echo "  lint-backend           - Run clippy on backend"
	@echo "  lint-frontend          - Run ESLint on frontend"
	@echo "  format                 - Format all code + docs"
	@echo "  format-check           - Check formatting (no changes)"
	@echo "  typecheck              - TypeScript type checking"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Documentation ══════════════════════════════════════════════════$(RESET)"
	@echo "  lint-md                - Lint Markdown files"
	@echo "  lint-yaml              - Lint YAML files"
	@echo "  lint-docs              - Lint all docs (Markdown + YAML)"
	@echo "  format-md              - Format Markdown files"
	@echo "  format-yaml            - Format YAML files"
	@echo "  format-docs            - Format all docs (Markdown + YAML)"
	@echo "  links-check            - Check internal links in Markdown"
	@echo "  links-check-external   - Check external links (slow)"
	@echo "  links-check-all        - Check all links"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ CI Pipeline ════════════════════════════════════════════════════$(RESET)"
	@echo "  ci                     - Full CI pipeline (format + lint + typecheck + test)"
	@echo "  ci-full                - CI + link checking + E2E tests"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Database ═══════════════════════════════════════════════════════$(RESET)"
	@echo "  migrate                - Run database migrations"
	@echo "  migrate-create name=x  - Create a new migration"
	@echo "  migrate-revert         - Revert the last migration"
	@echo "  db-seed                - Load test seed data (dev/test only)"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Docker — Infrastructure ════════════════════════════════════════$(RESET)"
	@echo "  docker-infra           - Start dev infrastructure (services depend on LLM)"
	@echo "  docker-infra-down      - Stop dev infrastructure"
	@echo "  docker-infra-restart   - Restart dev infrastructure"
	@echo "  docker-infra-logs      - Tail infrastructure container logs"
	@echo "  docker-infra-ps        - Show infrastructure container status"
	@echo "  docker-infra-health    - Health check infrastructure containers"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Docker — Production ════════════════════════════════════════════$(RESET)"
	@echo "  docker-up              - Start full production stack"
	@echo "  docker-down            - Stop production stack"
	@echo "  docker-logs            - Tail production logs"
	@echo "  docker-build           - Build Docker images"
	@echo "  docker-build-no-cache  - Build images without cache"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Docker — Testing ═══════════════════════════════════════════════$(RESET)"
	@echo "  docker-test-up         - Start test database (port 5433)"
	@echo "  docker-test-down       - Stop test database"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Dependencies ═══════════════════════════════════════════════════$(RESET)"
	@echo "  outdated               - Show outdated dependencies"
	@echo "  upgrade                - Upgrade dependencies within semver"
	@echo "  audit                  - Security audit all dependencies"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Coverage & Quality ═════════════════════════════════════════════$(RESET)"
	@echo "  coverage               - Generate test coverage report (cargo-llvm-cov)"
	@echo "  deadcode               - Check for dead code"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Infrastructure ═════════════════════════════════════════════════$(RESET)"
	@echo "  minio                  - Start MinIO object storage"
	@echo "  backup                 - Run database backup manually"
	@echo "  observability          - Start SigNoz observability stack"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ AI & Models ═════════════════════════════════════════════════════$(RESET)"
	@echo "  dev-candle             - Start with Candle LLM (auto-detects GPU)"
	@echo "  dev-ollama             - Start with Ollama LLM"
	@echo "  dev-no-llm             - Start without any LLM (Tiers 0-2 only)"
	@echo "  models                 - List downloaded models (set LLM=candle or ollama)"
	@echo "  download-model         - Download the default model (set LLM=candle or ollama)"
	@echo "  check-ollama           - Diagnose Ollama setup (local vs Docker)"
	@echo ""
	@echo "  Run '$(BOLD)make -C frontend$(RESET)' for frontend-specific targets."

# ═══════════════════════════════════════════════════════════════
#  Install & Build
# ═══════════════════════════════════════════════════════════════

.PHONY: install build build-release start dev dev-backend dev-watch

install: ## Install all dependencies (backend + frontend)
	cargo fetch
	$(MAKE) -C $(FRONTEND_DIR) install

build: ## Build all (backend debug + frontend)
	cargo build --workspace
	$(MAKE) -C $(FRONTEND_DIR) build

build-release: ## Build backend in release mode
	cargo build --release -p finima-api $(CARGO_LLM_FEATURES)

start: docker-infra ## Start everything (infra + backend + frontend)
	@echo "$(GREEN)Waiting for infrastructure to be healthy...$(RESET)"
	@for i in $$(seq 1 30); do \
		pg_isready -h localhost -p 5432 -U finima >/dev/null 2>&1 && break; \
		sleep 1; \
	done
	@$(MAKE) dev

dev: ## Start backend + frontend (assumes infra is running)
	@echo "$(GREEN)Backend: http://localhost:3000  Frontend: http://localhost:5173$(RESET)"
	@echo "$(CYAN)LLM backend: $(LLM)$(if $(filter ollama,$(LLM)), ($(OLLAMA_SOURCE)),)$(RESET)"
	@trap 'kill 0' INT TERM EXIT; \
		APP_ENV=development cargo run --bin finima-api $(CARGO_LLM_FEATURES) & \
		$(MAKE) -C $(FRONTEND_DIR) dev & \
		wait

dev-backend: ## Start backend API server only
	@echo "$(CYAN)LLM backend: $(LLM)$(RESET)"
	APP_ENV=development cargo run --bin finima-api $(CARGO_LLM_FEATURES)

dev-watch: ## Start backend with auto-reload (requires cargo-watch)
	@echo "$(CYAN)LLM backend: $(LLM)$(RESET)"
	APP_ENV=development cargo watch -x 'run --bin finima-api $(CARGO_LLM_FEATURES)'

# ═══════════════════════════════════════════════════════════════
#  Testing
# ═══════════════════════════════════════════════════════════════

.PHONY: test test-all test-unit test-integration test-llm test-frontend test-e2e

test: test-unit test-frontend ## Run all unit tests (backend + frontend)

test-all: docker-test-up test-all-backend test-frontend docker-test-down ## Run ALL tests (starts/stops test DB automatically)

test-all-backend: ## Run all backend tests (unit + integration, requires test DB)
	@echo "Waiting for test database..."
	@for i in 1 2 3 4 5 6 7 8 9 10; do \
		pg_isready -h localhost -p 5433 -U finima -d finima_test >/dev/null 2>&1 && break; \
		sleep 1; \
	done
	TEST_DATABASE_URL="postgres://finima:test@localhost:5433/finima_test" \
		cargo test --workspace

test-unit: ## Run backend unit tests (no database required)
	cargo test --workspace --lib

test-integration: ## Run backend integration tests (starts test DB if needed)
	@if ! pg_isready -h localhost -p 5433 -U finima -d finima_test >/dev/null 2>&1; then \
		echo "Starting test database..."; \
		$(COMPOSE_TEST) up -d postgres; \
		for i in 1 2 3 4 5 6 7 8 9 10; do \
			pg_isready -h localhost -p 5433 -U finima -d finima_test >/dev/null 2>&1 && break; \
			sleep 1; \
		done; \
	fi
	TEST_DATABASE_URL="postgres://finima:test@localhost:5433/finima_test" \
		APP_ENV=test cargo test --workspace --test '*'

OLLAMA_TEST_PORT  ?= 11435
OLLAMA_TEST_MODEL ?= gemma4:e4b-it-q4_K_M

test-llm: ## Run LLM integration tests (auto-starts Ollama, pulls model)
	@echo "Starting Ollama test container..."
	@$(COMPOSE_TEST) up -d ollama
	@echo "Waiting for Ollama to be ready..."
	@for i in $$(seq 1 30); do \
		curl -sf http://localhost:$(OLLAMA_TEST_PORT)/api/version >/dev/null 2>&1 && break; \
		sleep 2; \
	done
	@echo "Pulling test model $(OLLAMA_TEST_MODEL) (this may take a while on first run)..."
	@curl -sf http://localhost:$(OLLAMA_TEST_PORT)/api/pull -d '{"name":"$(OLLAMA_TEST_MODEL)"}' \
		| while read -r line; do \
			status=$$(echo "$$line" | grep -o '"status":"[^"]*"' | head -1); \
			printf "\r  %s" "$$status"; \
		done; echo ""
	@echo "Running LLM integration tests..."
	OLLAMA_URL="http://localhost:$(OLLAMA_TEST_PORT)" \
	OLLAMA_TEST_MODEL="$(OLLAMA_TEST_MODEL)" \
		cargo test -p finima-llm --features ollama -- --ignored
	@echo "LLM tests complete."

test-frontend: ## Run frontend unit tests
	$(MAKE) -C $(FRONTEND_DIR) test

test-e2e: ## Run end-to-end tests (requires running backend)
	$(MAKE) -C $(FRONTEND_DIR) test-e2e

# ═══════════════════════════════════════════════════════════════
#  Lint & Format — Code
# ═══════════════════════════════════════════════════════════════

.PHONY: lint lint-backend lint-frontend format format-check typecheck

lint: lint-docs lint-backend lint-frontend ## Lint everything (code + docs)

lint-backend: ## Run clippy on backend
	cargo clippy --workspace --all-targets -- -D warnings

lint-frontend: ## Run ESLint on frontend
	$(MAKE) -C $(FRONTEND_DIR) lint

format: format-docs ## Format all code + docs
	cargo fmt --all
	$(MAKE) -C $(FRONTEND_DIR) format

format-check: format-check-docs ## Check formatting (code + docs)
	cargo fmt --all -- --check
	$(MAKE) -C $(FRONTEND_DIR) format-check

typecheck: ## TypeScript type checking
	$(MAKE) -C $(FRONTEND_DIR) typecheck

# ═══════════════════════════════════════════════════════════════
#  Lint & Format — Documentation
# ═══════════════════════════════════════════════════════════════

.PHONY: lint-md lint-yaml lint-docs format-md format-yaml format-docs format-check-md format-check-yaml format-check-docs

lint-md: ## Lint Markdown files
	@echo "$(GREEN)Linting Markdown...$(RESET)"
	@if command -v markdownlint-cli2 >/dev/null 2>&1; then \
		markdownlint-cli2 '**/*.md' '#**/node_modules' '#**/target' '#.claude/worktrees/**' || true; \
	else \
		echo "$(YELLOW)markdownlint-cli2 not installed. Run: npm i -g markdownlint-cli2$(RESET)"; \
	fi

lint-yaml: ## Lint YAML files
	@echo "$(GREEN)Linting YAML...$(RESET)"
	@find . $(PRUNE_DIRS) -o \( -name '*.yaml' -o -name '*.yml' \) ! -name 'pnpm-lock.yaml' -print | \
		xargs yamllint -c .yamllint.yaml 2>/dev/null || \
		echo "$(YELLOW)yamllint not installed. Run: pip install yamllint$(RESET)"

lint-docs: lint-md lint-yaml ## Lint all docs (Markdown + YAML)

format-md: ## Format Markdown files
	@find . $(PRUNE_DIRS) -o -name '*.md' -print | xargs npx prettier --write --no-error-on-unmatched-pattern

format-yaml: ## Format YAML files
	@find . $(PRUNE_DIRS) -o \( -name '*.yaml' -o -name '*.yml' \) ! -name 'pnpm-lock.yaml' -print | \
		xargs npx prettier --write --no-error-on-unmatched-pattern

format-docs: format-md format-yaml ## Format all docs (Markdown + YAML)

format-check-md: ## Check Markdown formatting
	@find . $(PRUNE_DIRS) -o -name '*.md' -print | xargs npx prettier --check --no-error-on-unmatched-pattern

format-check-yaml: ## Check YAML formatting
	@find . $(PRUNE_DIRS) -o \( -name '*.yaml' -o -name '*.yml' \) ! -name 'pnpm-lock.yaml' -print | \
		xargs npx prettier --check --no-error-on-unmatched-pattern

format-check-docs: format-check-md format-check-yaml ## Check doc formatting

# ═══════════════════════════════════════════════════════════════
#  Link Checking
# ═══════════════════════════════════════════════════════════════

.PHONY: links-check links-check-external links-check-all

links-check: ## Check internal links in Markdown
	@echo "$(GREEN)Checking local file links...$(RESET)"
	@if [ -n "$(LYCHEE)" ]; then \
		$(LYCHEE) --scheme file --include-fragments --config .lychee.toml '**/*.md'; \
	else \
		echo "$(YELLOW)lychee not installed. Run: cargo install lychee$(RESET)"; \
	fi

links-check-external: ## Check external links (may take minutes)
	@echo "$(GREEN)Checking external links...$(RESET)"
	@if [ -n "$(LYCHEE)" ]; then \
		$(LYCHEE) --scheme https --scheme http --config .lychee.toml '**/*.md'; \
	else \
		echo "$(YELLOW)lychee not installed. Run: cargo install lychee$(RESET)"; \
	fi

links-check-all: links-check links-check-external ## Check all links (internal + external)

# ═══════════════════════════════════════════════════════════════
#  CI Pipeline
# ═══════════════════════════════════════════════════════════════

.PHONY: ci ci-full

ci: format-check lint typecheck test ## Full CI pipeline (format + lint + typecheck + test)

ci-full: ci links-check test-e2e ## CI + link checking + E2E tests

# ═══════════════════════════════════════════════════════════════
#  Database
# ═══════════════════════════════════════════════════════════════

.PHONY: migrate migrate-create migrate-revert db-seed

migrate: ## Run database migrations
	sqlx migrate run --source crates/finima-db/src/migrations

migrate-create: ## Create a new migration (usage: make migrate-create name=add_foo)
	sqlx migrate add -r $(name) --source crates/finima-db/src/migrations

migrate-revert: ## Revert the last migration
	sqlx migrate revert --source crates/finima-db/src/migrations

db-seed: ## Load test seed data (dev/test only)
	@if [ "$${APP_ENV}" = "production" ]; then \
		echo "ERROR: Cannot seed production database"; exit 1; \
	fi
	psql "$${DATABASE_URL:-postgres://finima:finima_dev@localhost:5432/finima}" -f tests/seed.sql

# ═══════════════════════════════════════════════════════════════
#  Docker — Infrastructure
# ═══════════════════════════════════════════════════════════════

.PHONY: docker-infra docker-infra-down docker-infra-restart docker-infra-logs docker-infra-ps docker-infra-health

docker-infra: ## Start dev infrastructure (services depend on LLM setting)
ifeq ($(LLM),ollama)
ifeq ($(OLLAMA_SOURCE),local)
	@echo "$(GREEN)Ollama: using local instance on port $(OLLAMA_PORT)$(RESET)"
	@if docker inspect -f '{{.State.Running}}' finima-ollama 2>/dev/null | grep -q true; then \
		echo "$(YELLOW)Stopping Docker Ollama (finima-ollama) to avoid port conflict...$(RESET)"; \
		docker stop finima-ollama >/dev/null 2>&1; \
	fi
else
	@echo "$(CYAN)Ollama: starting Docker container (no local instance detected)$(RESET)"
endif
endif
	$(COMPOSE_DEV) up -d $(INFRA_SERVICES)

docker-infra-down: ## Stop dev infrastructure
	$(COMPOSE_DEV) down

docker-infra-restart: ## Restart dev infrastructure
	$(COMPOSE_DEV) restart

docker-infra-logs: ## Tail infrastructure container logs
	$(COMPOSE_DEV) logs -f

docker-infra-ps: ## Show infrastructure container status
	$(COMPOSE_DEV) ps

docker-infra-health: ## Health check infrastructure containers
	@$(COMPOSE_DEV) ps --format '{{.Name}}\t{{.Status}}' | column -t

# ═══════════════════════════════════════════════════════════════
#  Docker — Production
# ═══════════════════════════════════════════════════════════════

.PHONY: docker-up docker-down docker-logs docker-build docker-build-no-cache

docker-up: ## Start full production stack
	$(COMPOSE_PROD) up -d

docker-down: ## Stop production stack
	$(COMPOSE_PROD) down

docker-logs: ## Tail production logs
	$(COMPOSE_PROD) logs -f

docker-build: ## Build Docker images
	docker build -t finima-backend -f Dockerfile.backend .
	docker build -t finima-frontend -f frontend/Dockerfile.frontend frontend/

docker-build-no-cache: ## Build Docker images without cache
	docker build --no-cache -t finima-backend -f Dockerfile.backend .
	docker build --no-cache -t finima-frontend -f frontend/Dockerfile.frontend frontend/

# ═══════════════════════════════════════════════════════════════
#  Docker — Testing
# ═══════════════════════════════════════════════════════════════

.PHONY: docker-test-up docker-test-down

docker-test-up: ## Start test database (port 5433)
	$(COMPOSE_TEST) up -d

docker-test-down: ## Stop test database
	$(COMPOSE_TEST) down

# ═══════════════════════════════════════════════════════════════
#  Dependencies
# ═══════════════════════════════════════════════════════════════

.PHONY: outdated upgrade audit

outdated: ## Show outdated dependencies
	cargo outdated -R --workspace 2>/dev/null || echo "Install: cargo install cargo-outdated"
	$(MAKE) -C $(FRONTEND_DIR) outdated

upgrade: ## Upgrade dependencies within semver
	cargo update
	$(MAKE) -C $(FRONTEND_DIR) upgrade

audit: ## Security audit all dependencies
	cargo audit 2>/dev/null || echo "Install: cargo install cargo-audit"
	$(MAKE) -C $(FRONTEND_DIR) audit

# ═══════════════════════════════════════════════════════════════
#  Coverage & Quality
# ═══════════════════════════════════════════════════════════════

.PHONY: coverage deadcode

coverage: ## Generate test coverage report (requires cargo-llvm-cov)
	cargo llvm-cov --workspace --lib --html
	@echo "Coverage report: target/llvm-cov/html/index.html"

deadcode: ## Check for dead code
	cargo clippy --workspace -- -W dead-code
	$(MAKE) -C $(FRONTEND_DIR) deadcode

# ═══════════════════════════════════════════════════════════════
#  LLM / AI Models
# ═══════════════════════════════════════════════════════════════

.PHONY: dev-candle dev-ollama dev-no-llm models download-model check-ollama

dev-candle: ## Start with Candle in-process LLM (auto-detects Metal/CUDA/CPU)
	@$(MAKE) dev LLM=candle

dev-ollama: ## Start with Ollama HTTP LLM
	@$(MAKE) dev LLM=ollama

dev-no-llm: ## Start without any LLM (Tiers 0-2 only)
	@$(MAKE) dev LLM=none

models: ## List downloaded models (set LLM=candle or LLM=ollama)
ifeq ($(filter candle candle-metal candle-cuda,$(LLM)),)
ifeq ($(LLM),ollama)
	@if docker inspect -f '{{.State.Running}}' finima-ollama 2>/dev/null | grep -q true; then \
		echo "$(CYAN)Models in Docker Ollama (finima-ollama):$(RESET)"; \
		docker exec finima-ollama ollama list; \
	fi
	@if command -v ollama >/dev/null 2>&1 && ollama list >/dev/null 2>&1; then \
		echo "$(CYAN)Models in local Ollama:$(RESET)"; \
		ollama list; \
	fi
	@if ! docker inspect -f '{{.State.Running}}' finima-ollama 2>/dev/null | grep -q true \
		&& ! (command -v ollama >/dev/null 2>&1 && ollama list >/dev/null 2>&1); then \
		echo "No Ollama instance found. Start with: make docker-infra LLM=ollama"; \
	fi
else
	$(error Set LLM to candle or ollama (current: $(LLM)))
endif
else
	@HF_CACHE="$${HF_HOME:-$${HOME}/.cache/huggingface}/hub"; \
	if [ -d "$$HF_CACHE" ]; then \
		found=0; \
		for d in "$$HF_CACHE"/models--*; do \
			[ -d "$$d" ] || continue; \
			name=$$(basename "$$d" | sed 's/^models--//; s/--/\//g'); \
			gguf_count=$$(find "$$d" -name '*.gguf' 2>/dev/null | wc -l | tr -d ' '); \
			printf "  %-45s (%s GGUF files)\n" "$$name" "$$gguf_count"; \
			found=1; \
		done; \
		[ "$$found" = "1" ] || echo "No models found in $$HF_CACHE"; \
	else \
		echo "HuggingFace cache not found at $$HF_CACHE"; \
		echo "Run 'make download-model' to download a model."; \
	fi
endif

download-model: ## Download the default model (set LLM=candle or LLM=ollama)
ifeq ($(filter candle candle-metal candle-cuda,$(LLM)),)
ifeq ($(LLM),ollama)
	@if [ "$(OLLAMA_SOURCE)" = "docker" ]; then \
		echo "$(CYAN)Pulling $(OLLAMA_MODEL) into Docker Ollama...$(RESET)"; \
		docker exec finima-ollama ollama pull $(OLLAMA_MODEL); \
	elif [ "$(OLLAMA_SOURCE)" = "local" ]; then \
		echo "$(CYAN)Pulling $(OLLAMA_MODEL) into local Ollama...$(RESET)"; \
		ollama pull $(OLLAMA_MODEL); \
	else \
		echo "No Ollama instance found. Start with: make docker-infra LLM=ollama"; \
		exit 1; \
	fi
else
	$(error Set LLM to candle or ollama (current: $(LLM)))
endif
else
	cargo run -p finima-llm --features candle --bin download_model
endif

check-ollama: ## Diagnose Ollama setup (local vs Docker, model availability)
	@echo "$(BOLD)Ollama Diagnostics$(RESET)"
	@echo ""
	@echo "$(BOLD)Local Ollama:$(RESET)"
	@if command -v ollama >/dev/null 2>&1; then \
		ver=$$(ollama --version 2>/dev/null || echo "unknown"); \
		echo "  Installed: yes ($$ver)"; \
		if ollama list >/dev/null 2>&1; then \
			echo "  Status:    running"; \
			echo "  Models:"; \
			ollama list 2>/dev/null | sed 's/^/    /'; \
		else \
			echo "  Status:    not running"; \
		fi; \
	else \
		echo "  Installed: no"; \
	fi
	@echo ""
	@echo "$(BOLD)Docker Ollama (finima-ollama):$(RESET)"
	@if docker inspect -f '{{.State.Running}}' finima-ollama 2>/dev/null | grep -q true; then \
		echo "  Status:    running"; \
		port=$$(docker port finima-ollama 11434 2>/dev/null | head -1); \
		echo "  Port:      $$port"; \
		echo "  Models:"; \
		docker exec finima-ollama ollama list 2>/dev/null | sed 's/^/    /'; \
	else \
		echo "  Status:    not running"; \
	fi
	@echo ""
	@echo "$(BOLD)Backend connects to:$(RESET) http://localhost:$(OLLAMA_PORT)"
	@if curl -sf http://localhost:$(OLLAMA_PORT)/api/version >/dev/null 2>&1; then \
		echo "  Port $(OLLAMA_PORT): $(GREEN)responding$(RESET)"; \
		if curl -sf http://localhost:$(OLLAMA_PORT)/api/tags 2>/dev/null \
			| grep -q '"$(OLLAMA_MODEL)"'; then \
			echo "  Model $(OLLAMA_MODEL): $(GREEN)available$(RESET)"; \
		else \
			echo "  Model $(OLLAMA_MODEL): $(YELLOW)NOT FOUND$(RESET)"; \
			echo "  Run: make download-model LLM=ollama"; \
		fi; \
	else \
		echo "  Port $(OLLAMA_PORT): $(YELLOW)not responding$(RESET)"; \
	fi

# ═══════════════════════════════════════════════════════════════
#  Infrastructure (MinIO, Backups, Observability)
# ═══════════════════════════════════════════════════════════════

.PHONY: minio backup observability

minio: ## Start MinIO object storage
	$(COMPOSE_DEV) up -d minio

backup: ## Run database backup manually
	$(COMPOSE_PROD) run --rm backup /scripts/backup.sh

observability: ## Start SigNoz observability stack
	$(COMPOSE_OBS) up -d

# ═══════════════════════════════════════════════════════════════
#  Clean
# ═══════════════════════════════════════════════════════════════

.PHONY: clean clean-all

clean: ## Clean build artifacts
	cargo clean
	$(MAKE) -C $(FRONTEND_DIR) clean

clean-all: clean ## Clean build + Docker volumes (DESTROYS DATA)
	$(COMPOSE_DEV) down -v 2>/dev/null || true
	$(COMPOSE_PROD) down -v 2>/dev/null || true
	$(COMPOSE_TEST) down -v 2>/dev/null || true
