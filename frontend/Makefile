# ============================================================================
# Finima — Frontend Makefile
# ============================================================================
# React/Vite frontend application.
#
# Quick Start:
#   make help         - Show all available targets
#   make install      - Install dependencies
#   make dev          - Start Vite dev server
#   make test         - Run unit tests
# ============================================================================

SHELL := /bin/bash
PNPM  := pnpm

.DEFAULT_GOAL := help

# Colors
BOLD   := $(shell tput bold 2>/dev/null || echo '')
GREEN  := $(shell tput setaf 2 2>/dev/null || echo '')
YELLOW := $(shell tput setaf 3 2>/dev/null || echo '')
BLUE   := $(shell tput setaf 4 2>/dev/null || echo '')
RESET  := $(shell tput sgr0 2>/dev/null || echo '')

# ============================================================================
# Default Target
# ============================================================================

.PHONY: help
help:
	@echo "$(BOLD)$(BLUE)╔════════════════════════════════════════════════════════════════════╗$(RESET)"
	@echo "$(BOLD)$(BLUE)║                    Finima Frontend Makefile                        ║$(RESET)"
	@echo "$(BOLD)$(BLUE)╚════════════════════════════════════════════════════════════════════╝$(RESET)"
	@echo ""
	@echo "$(BOLD)Quick Start:$(RESET)"
	@echo "  make install           - Install dependencies via pnpm"
	@echo "  make dev               - Start Vite dev server"
	@echo "  make build             - Build for production"
	@echo "  make test              - Run unit tests"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Install & Build ═════════════════════════════════════════════════$(RESET)"
	@echo "  install                - Install dependencies via pnpm"
	@echo "  build                  - Build for production (typecheck + vite build)"
	@echo "  dev                    - Start Vite dev server"
	@echo "  preview                - Preview production build locally"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Test ════════════════════════════════════════════════════════════$(RESET)"
	@echo "  test                   - Run unit tests (single run)"
	@echo "  test-watch             - Run unit tests in watch mode"
	@echo "  test-e2e               - Run Playwright E2E tests"
	@echo "  test-e2e-ui            - Run E2E tests with Playwright UI"
	@echo "  test-e2e-headed        - Run E2E tests in headed browser"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Lint & Format ═══════════════════════════════════════════════════$(RESET)"
	@echo "  lint                   - Run ESLint"
	@echo "  format                 - Format code with Prettier"
	@echo "  format-check           - Check formatting without changes"
	@echo "  typecheck              - TypeScript type checking"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Dependencies ═══════════════════════════════════════════════════$(RESET)"
	@echo "  outdated               - Show outdated dependencies"
	@echo "  upgrade                - Upgrade dependencies within semver"
	@echo "  upgrade-latest         - Upgrade dependencies to latest (may break semver)"
	@echo "  audit                  - Security audit production dependencies"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Quality ════════════════════════════════════════════════════════$(RESET)"
	@echo "  deadcode               - Check for unused exports (requires ts-prune)"
	@echo ""
	@echo "$(BOLD)$(BLUE)═══ Clean ══════════════════════════════════════════════════════════$(RESET)"
	@echo "  clean                  - Clean build artifacts"
	@echo "  clean-deps             - Remove node_modules (full reinstall needed)"

# ═══════════════════════════════════════════════════════════════
#  Install & Build
# ═══════════════════════════════════════════════════════════════

.PHONY: install build dev preview

install: ## Install dependencies via pnpm
	$(PNPM) install

build: ## Build for production (typecheck + vite build)
	$(PNPM) run build

dev: ## Start Vite dev server
	$(PNPM) run dev

preview: ## Preview production build locally
	$(PNPM) run preview

# ═══════════════════════════════════════════════════════════════
#  Testing
# ═══════════════════════════════════════════════════════════════

.PHONY: test test-watch test-e2e test-e2e-ui test-e2e-headed

test: ## Run unit tests (single run)
	$(PNPM) run test -- --run

test-watch: ## Run unit tests in watch mode
	$(PNPM) run test

test-e2e: ## Run Playwright E2E tests
	$(PNPM) exec playwright test

test-e2e-ui: ## Run E2E tests with Playwright UI
	$(PNPM) exec playwright test --ui

test-e2e-headed: ## Run E2E tests in headed browser
	$(PNPM) exec playwright test --headed

# ═══════════════════════════════════════════════════════════════
#  Lint & Format
# ═══════════════════════════════════════════════════════════════

.PHONY: lint format format-check typecheck

lint: ## Run ESLint
	$(PNPM) run lint

format: ## Format code with Prettier
	$(PNPM) run fmt

format-check: ## Check formatting without changes
	$(PNPM) exec prettier --check 'src/**/*.{ts,tsx,css}'

typecheck: ## TypeScript type checking
	$(PNPM) exec tsc --noEmit

# ═══════════════════════════════════════════════════════════════
#  Dependencies
# ═══════════════════════════════════════════════════════════════

.PHONY: outdated upgrade upgrade-latest audit

outdated: ## Show outdated dependencies
	$(PNPM) outdated

upgrade: ## Upgrade dependencies within semver
	$(PNPM) update

upgrade-latest: ## Upgrade dependencies to latest (may break semver)
	$(PNPM) update --latest

audit: ## Security audit production dependencies
	$(PNPM) audit --prod

# ═══════════════════════════════════════════════════════════════
#  Quality
# ═══════════════════════════════════════════════════════════════

.PHONY: deadcode

deadcode: ## Check for unused exports (requires ts-prune)
	$(PNPM) exec ts-prune 2>/dev/null || echo "Install: pnpm add -D ts-prune"

# ═══════════════════════════════════════════════════════════════
#  Clean
# ═══════════════════════════════════════════════════════════════

.PHONY: clean clean-deps

clean: ## Clean build artifacts
	rm -rf dist .vite *.tsbuildinfo

clean-deps: ## Remove node_modules (full reinstall needed)
	rm -rf node_modules
