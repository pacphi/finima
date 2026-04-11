# Finima Frontend

React single-page application for Finima -- the privacy-first personal finance platform.

## Stack

- **React 19** with TypeScript
- **Vite 6** -- dev server and build tooling
- **Zustand 5** -- lightweight state management
- **Recharts** -- charting (donut, waterfall, Sankey, gauges)
- **TanStack Table** -- data tables with sorting, filtering, bulk edit
- **React Hook Form + Zod** -- form handling and validation
- **Tailwind CSS 4** -- utility-first styling
- **Vitest** -- unit testing
- **Playwright** -- end-to-end testing

## Directory Structure

```text
src/
  api/            API client modules (auth, portfolios, accounts, transactions, etc.)
  components/
    charts/       Visualization components (HealthScoreGauge, SankeyDiagram, etc.)
    layout/       App shell (AppLayout, Header, Sidebar)
    tables/       Transaction table, bulk edit bar, category cell
    ui/           Shared UI primitives (ThemeSwitcher, ColorPicker)
    upload/       File upload and column mapping modal
  hooks/          Custom hooks (useApi, useFocusTrap)
  routes/         Page components (Dashboard, Transactions, Budget, Flows, etc.)
  stores/         Zustand stores (auth, portfolio, config, prefs, theme)
  theme/          CSS variables and theming
  types/          TypeScript type definitions (models.ts)
  utils/          Formatting helpers (currency, dates)
```

## Development

From the project root (preferred):

```bash
make -C frontend install   # Install deps
make -C frontend dev       # Start dev server
make -C frontend build     # Production build
```

Or directly:

```bash
pnpm install
pnpm dev          # Start dev server on http://localhost:5173
pnpm build        # Type-check and production build
pnpm preview      # Preview production build locally
```

## Testing

From the project root (preferred):

```bash
make test-frontend            # Unit tests
make -C frontend e2e          # End-to-end tests (requires E2E_ENABLED=true)
```

Or directly:

```bash
pnpm test         # Unit tests (Vitest)
pnpm e2e          # End-to-end tests (Playwright, requires E2E_ENABLED)
pnpm e2e:headed   # E2E in headed Chromium for debugging
```

Playwright tests require a running backend. See the root [Quick Start](../docs/guides/quick-start.md) for setup.

## Linting and Formatting

```bash
pnpm lint         # ESLint
pnpm fmt          # Prettier
pnpm typecheck    # TypeScript type checking (no emit)
```

## Key Patterns

**API factory pattern** -- Each domain has a module in `src/api/` that exports typed request functions. These are consumed through the `useApi` hook which handles loading state, errors, and auth headers.

**Preferences store** -- `src/stores/prefsStore.ts` holds user formatting preferences (currency, date format, locale). All display formatting flows through this store.

**Authentication** -- Session tokens are stored in `sessionStorage`. The `authStore` manages sign-in state and token refresh. Magic-link flow handles passwordless auth.

**Theming** -- `themeStore` manages dark/light mode. CSS variables in `src/theme/variables.css` define the color system.

## Environment Configuration

The frontend reads runtime configuration from `config.yaml` (loaded at startup via `configStore`). The Vite dev server proxies API requests based on `VITE_API_BASE_URL` (defaults to `http://localhost:3000`).

## Top of Mind for Developers

**Formatting** -- Always use `formatCurrency` and `formatDate` from `src/utils/format.ts`. Never hardcode currency symbols, decimal separators, or date patterns. These functions respect the user's locale and preferences.

**Modals** -- All modal components must use the `useFocusTrap` hook from `src/hooks/useFocusTrap.ts` to trap keyboard focus inside the dialog while open.

**Error handling** -- Never swallow errors silently. Every API call should surface failures to the user via toast notifications or inline error states. Use the `ErrorBoundary` component to catch rendering errors.

**Accessibility** -- Maintain WCAG 2.1 AA compliance. This means: proper ARIA roles and labels on interactive elements, full keyboard navigation support, sufficient color contrast, and never relying on color alone to convey information.
