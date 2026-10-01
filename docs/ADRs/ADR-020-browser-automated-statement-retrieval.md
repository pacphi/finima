# ADR-020: Browser-Assisted Transaction Export Retrieval (finima-fetch)

**Status:** Proposed  
**Date:** 2026-10-01  
**Deciders:** Chris Phillipson

---

## Context

Finima imports transactions only from files the user downloads from each bank's website and uploads (ADR-005). With about a dozen institutions, that monthly chore is the main barrier to keeping finima current. ADR-005 rejected Plaid-style aggregators because they break the privacy-first design, cost money and are fragile. Any automation has to keep that property: no third party between the user and their bank.

## Decision

Add **finima-fetch**, a local TypeScript CLI in `fetcher/` with a Claude Code skill in front of it:

- **Browser connection.** The CLI starts the user's installed **Chromium-family browser** (Brave by default; Chrome, Edge and Chromium also supported) as a normal detached process with a dedicated profile and a loopback-only debugging port. It attaches with `puppeteer-core` over CDP for each command.
- **Login.** The **human logs in** to each institution in its own tab. The tool never handles credentials, MFA codes or full account numbers.
- **Adapters.** Each institution has a **deterministic adapter**: TypeScript steps plus externalized selectors, per ADR-009. Adapters export transactions for a calendar-month or arbitrary date range (ADR-016) in a format finima already parses (ADR-005).
- **LLM boundary.** Claude is used only to turn user requests into CLI calls (the skill) and to explore sites while writing adapters (training). Training sees **redacted accessibility trees only**. Normal runs make no LLM calls.
- **Output.** Files are written to `~/Downloads/finima/<institution>/<alias>/` (OS-aware), with a fetch-log. A later iteration uploads them through the existing `/api/uploads` routes.

The full design and the iteration roadmap are in [docs/superpowers/specs/2026-10-01-finima-fetch-design.md](../superpowers/specs/2026-10-01-finima-fetch-design.md).

## Consequences

**Positive:**

- Removes the manual download step, with no aggregator and no data leaving the machine except redacted training context sent to Claude.
- Needs no changes to finima's backend until the optional import iteration.
- Adapters are reproducible code with fixture tests, rather than a model improvising on every run.

**Negative:**

- Adapters break when banks redesign their sites. Mitigated by step-level diagnostics, externalized selectors, fixtures and a canary check.
- Bank bot detection or online-banking agreements may restrict automated access. Mitigated by a human login, human pacing, sequential jobs and no retried submissions. A pre-build spike checks this on Bank of America before any further work.
- While a session is open, local processes could attach to the logged-in browser through the debugging port. Mitigated by loopback only, a random port, a 0600 state file and an explicit `session end`.
- The README's "no cloud API calls, ever" needs a scoped exception for the optional skill and training path.

## Alternatives Considered

1. **Playwright with a persistent context.** It has better locators and `codegen`, but Node must hold the browser for the whole session, and Brave isn't officially supported. Kept as the fallback if the spike shows CDP attach triggers bot detection.
2. **Claude in Chrome as the runtime driver.** Rejected: every run would go through the LLM, full page content would go to the cloud without redaction we control, and runs wouldn't be repeatable.
3. **Browser extension covering Firefox and Safari as well.** Rejected for scope. The user chose Chromium-only.
4. **Cloning the user's everyday browser profile** (the tub-vault approach). Rejected because far more would be exposed if something went wrong; a fresh profile holds only bank sessions.
