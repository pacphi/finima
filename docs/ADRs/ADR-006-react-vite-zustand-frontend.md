# ADR-006: React + Vite + Zustand Frontend Stack

**Status:** Accepted  
**Date:** 2026-04-10  
**Deciders:** Chris Phillipson

---

## Context

Finima's frontend must render complex financial dashboards (charts, tables, drag-and-drop layouts), handle real-time WebSocket updates (import progress, LLM status), support theming (light/dark/custom accent), and provide responsive UX for desktop and mobile browsers.

## Decision

Build the frontend with:

| Layer            | Choice                                      | Rationale                                                                                                        |
| ---------------- | ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Framework        | **React 19**                                | Largest ecosystem, widest library compatibility (Recharts, TanStack Table, react-grid-layout).                   |
| Build tool       | **Vite**                                    | Fast HMR, minimal config, native ESM.                                                                            |
| State management | **Zustand**                                 | Minimal boilerplate, no providers, performant selectors. Stores: auth, theme, preferences, portfolio, WebSocket. |
| Routing          | **React Router v7**                         | De facto standard, nested layouts, loader patterns.                                                              |
| Styling          | **Tailwind CSS v4** + CSS custom properties | Utility-first for rapid development. Custom properties for runtime theme switching.                              |
| Charts           | **Recharts**                                | React-native, composable, good for line/bar/donut/area charts.                                                   |
| Tables           | **TanStack Table v8**                       | Headless, sorting/filtering/pagination built-in, virtualizes large datasets.                                     |
| Drag-and-drop    | **react-grid-layout**                       | Dashboard widget rearrangement with grid snapping and persistence.                                               |
| File upload      | **react-dropzone**                          | Drop zone with file type validation, established library.                                                        |
| Forms            | **React Hook Form + Zod**                   | Performant uncontrolled forms + type-safe schema validation.                                                     |
| HTTP client      | **Fetch API + custom hooks**                | No Axios dependency; custom `useApi` hook handles JWT refresh, error handling.                                   |

**State architecture:**

- 5 Zustand stores (auth, theme, preferences, portfolio, WebSocket) — small, focused, independently subscribable.
- No global Redux-style store. Each store manages its own domain.
- WebSocket store wraps a single connection, dispatches messages to relevant stores.

## Consequences

**Positive:**

- All chosen libraries are mature, well-maintained, and have strong TypeScript support.
- Zustand's minimal API reduces state management boilerplate compared to Redux/MobX.
- Tailwind + CSS custom properties enable runtime theme switching without CSS-in-JS overhead.
- Recharts composes naturally with React — no imperative D3 wrangling for standard chart types.
- react-grid-layout provides drag-and-drop dashboard customization out of the box.

**Negative:**

- Recharts lacks a built-in Sankey diagram component. Will need `recharts` for most charts + a separate library or custom SVG for Sankey visualization.
- Tailwind's utility classes can make JSX verbose. Mitigated: extract component-level classes into Tailwind `@apply` or component abstractions.
- No SSR/SSG — Vite SPA only. Acceptable for a self-hosted app where SEO is irrelevant.

## Alternatives Considered

1. **Next.js** — SSR/SSG capabilities are unnecessary for a self-hosted finance app. Added complexity. Rejected.
2. **Svelte/SvelteKit** — Excellent DX but smaller ecosystem for financial chart/table libraries. Rejected.
3. **Vue 3** — Viable but team familiarity and library ecosystem favor React. Rejected.
4. **Redux Toolkit** — More ceremony than Zustand for this app's state complexity. Rejected.
5. **D3.js (direct)** — Maximum flexibility for charts but imperative API clashes with React's declarative model. Deferred: may use D3 directly for Sankey only.
