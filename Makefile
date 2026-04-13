# ============================================================================
# Finima — Root Makefile
# ============================================================================
# Full-stack financial intelligence platform.
# Rust backend + React/Vite frontend.
#
# Quick Start:
#   make help              - Show all available targets
#   make install           - Install all dependencies
#   make dev               - Start backend API server
#   make docker-up         - Start dev services (PostgreSQL + Ollama)
#   make ci                - Run full CI pipeline
# ============================================================================

# ============================================================================
# Variables and Configuration
# ============================================================================

SHELL := /bin/bash
.DEFAULT_GOAL := help

BACKEND_DIR  := .
FRONTEND_DIR := frontend

COMPOSE      := docker compose

# Auto-detect NVIDIA GPU: include GPU overlay on Linux when nvidia-smi is available
HAS_NVIDIA   := $(shell command -v nvidia-smi >/dev/null 2>&1 && nvidia-smi >/dev/null 2>&1 && echo 1 || echo 0)
GPU_OVERLAY  := $(if $(filter 1,$(HAS_NVIDIA)), -f docker-compose.gpu.yml,)

COMPOSE_DEV  := $(COMPOSE) -f docker-compose.yml$(GPU_OVERLAY)
COMPOSE_PROD := $(COMPOSE) -f docker-compose.prod.yml$(GPU_OVERLAY)
COMPOSE_TEST := $(COMPOSE) -f docker-compose.test.yml
COMPOSE_OBS  := $(COMPOSE) -f docker-compose.yml -f docker-compose.observability.yml$(GPU_OVERLAY)

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
	@echo "  make dev               - Start backend API server"
	@echo "  make docker-up         - Start dev services (PostgreSQL + Ollama)"
	@echo "  make ci                - Run full CI pipeline"
	@echo "  make test              - Run all tests"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Install & Build ═════════════════════════════════════════════════$(RESET)"
	@echo "  install                - Install all dependencies (backend + frontend)"
	@echo "  build                  - Build all (backend debug + frontend)"
	@echo "  build-release          - Build backend in release mode"
	@echo "  dev                    - Start backend API server (development)"
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
	@echo "$(BOLD)$(BLUE)═══ Docker — Development ═══════════════════════════════════════════$(RESET)"
	@echo "  docker-up              - Start dev services (PostgreSQL + Ollama)"
	@echo "  docker-down            - Stop dev services"
	@echo "  docker-restart         - Restart dev services"
	@echo "  docker-logs            - Tail all container logs"
	@echo "  docker-logs-backend    - Tail backend logs"
	@echo "  docker-logs-frontend   - Tail frontend logs"
	@echo "  docker-ps              - Show container status"
	@echo "  docker-exec-backend    - Shell into backend container"
	@echo "  docker-health          - Health check all containers"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Docker — Production ════════════════════════════════════════════$(RESET)"
	@echo "  docker-prod            - Start production stack"
	@echo "  docker-prod-down       - Stop production stack"
	@echo "  docker-prod-logs       - Tail production logs"
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
	@echo "  models                 - List available Ollama models"
	@echo "  download-model         - Download default Gemma 4 model"
	@echo ""
	@echo "  Run '$(BOLD)make -C frontend$(RESET)' for frontend-specific targets."

# ═══════════════════════════════════════════════════════════════
#  Install & Build
# ═══════════════════════════════════════════════════════════════

.PHONY: install build build-release dev dev-watch

install: ## Install all dependencies (backend + frontend)
	cargo fetch
	$(MAKE) -C $(FRONTEND_DIR) install

build: ## Build all (backend debug + frontend)
	cargo build --workspace
	$(MAKE) -C $(FRONTEND_DIR) build

build-release: ## Build backend in release mode
	cargo build --release -p finima-api

dev: ## Start backend API server (development)
	APP_ENV=development cargo run --bin finima-api

dev-watch: ## Start backend with auto-reload (requires cargo-watch)
	APP_ENV=development cargo watch -x 'run --bin finima-api'

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
#  Docker — Development
# ═══════════════════════════════════════════════════════════════

.PHONY: docker-up docker-down docker-restart docker-logs docker-logs-backend docker-logs-frontend docker-ps docker-exec-backend docker-health

docker-up: ## Start dev services (PostgreSQL + Ollama)
	$(COMPOSE_DEV) up -d

docker-down: ## Stop dev services
	$(COMPOSE_DEV) down

docker-restart: ## Restart dev services
	$(COMPOSE_DEV) restart

docker-logs: ## Tail all container logs
	$(COMPOSE_DEV) logs -f

docker-logs-backend: ## Tail backend logs
	$(COMPOSE_DEV) logs -f backend 2>/dev/null || echo "Backend not running in Docker (use 'make dev' for local)"

docker-logs-frontend: ## Tail frontend logs
	$(COMPOSE_DEV) logs -f frontend 2>/dev/null || echo "Frontend not running in Docker (use 'make -C frontend dev')"

docker-ps: ## Show container status
	$(COMPOSE_DEV) ps

docker-exec-backend: ## Shell into backend container
	$(COMPOSE_DEV) exec backend /bin/bash

docker-health: ## Health check all containers
	@$(COMPOSE_DEV) ps --format '{{.Name}}\t{{.Status}}' | column -t

# ═══════════════════════════════════════════════════════════════
#  Docker — Production
# ═══════════════════════════════════════════════════════════════

.PHONY: docker-prod docker-prod-down docker-prod-logs docker-build docker-build-no-cache

docker-prod: ## Start production stack
	$(COMPOSE_PROD) up -d

docker-prod-down: ## Stop production stack
	$(COMPOSE_PROD) down

docker-prod-logs: ## Tail production logs
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

.PHONY: models download-model

models: ## List available Ollama models
	@ollama list 2>/dev/null || echo "Ollama not running. Start with: make docker-up"

download-model: ## Download default Gemma 4 model
	ollama pull gemma4:26b-a4b-it-q4_K_M

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
	$(COMPOSE_DEV) down -v
	$(COMPOSE_TEST) down -v 2>/dev/null || true
