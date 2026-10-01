# Spike: finima-fetch spawn-and-attach on Bank of America

**Status:** In progress
**Date:** 2026-10-
**Spec:** [finima-fetch design §3.1](../superpowers/specs/2026-10-01-finima-fetch-design.md)
**Decision affects:** [ADR-020](../ADRs/ADR-020-browser-automated-statement-retrieval.md) (Proposed)

## Environment

| Item                                           | Value                                                |
| ---------------------------------------------- | ---------------------------------------------------- |
| macOS                                          | 27.0.1                                               |
| Brave version (`probe`)                        |                                                      |
| Chromium major                                 |                                                      |
| `puppeteer-core`                               | 25.12.0                                              |
| Node                                           | v26.4.0                                              |
| Everyday Brave running?                        | Yes (separate instance expected; see Q1)             |
| Normal BofA login needs 2FA on a known device? | Yes: SMS code on every login, even on a known device |

## Pre-registered decision rules

| Outcome                                                                  | Decision                                                                                                                                                   |
| ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q1 passes, Q2 minimal passes, Q2 maximal passes                          | **GO.** D3 holds; default `connect` options are acceptable.                                                                                                |
| Q1 passes, Q2 minimal passes, Q2 maximal shows extra verification/blocks | **GO with constraint.** D3 holds; minimal connect options (`targetFilter`, `networkEnabled: false`, `issuesEnabled: false`) become mandatory in spec §5.1. |
| Q2 minimal fails (beyond normal new-device 2FA)                          | **NO-GO.** Stop; reopen option B (Playwright); add evidence to ADR-020 Alternatives.                                                                       |
| Q1 fails (no `DevToolsActivePort`, attach refused, or port not loopback) | **NO-GO** until explained; investigate before any other question.                                                                                          |
| Q3 `download-event` fails on BofA's export                               | GO stands; BofA declares `blob` or `in-page-fetch`; iteration 1 still builds all three capture modes.                                                      |

**Baseline rule (recorded 2026-10-01, before any run):** BofA sends an SMS code on every login, even on a known device. An SMS code prompt is therefore **not** a bot-detection signal in either pass. The signals are:

- an "unusual activity" or fraud-review page;
- a CAPTCHA;
- a lock or suspension warning;
- verification beyond the SMS code (security questions, a required call, a second code);
- an export that is refused or fails.

## Q1 — Attach works

## Q2 — Bot detection

### Pass B (minimal footprint, run first)

### Pass A (maximal footprint)

## Q3 — Download capture

## Q4 — Brave and the Chrome 136 default-profile restriction

## Q5 — Idle session timeout

## Extra: accessibility-tree size

## Extra: download-behavior scope

## Decision

## What iteration 1 should assume

- Toolchain pinned to the frontend's majors: TypeScript 6, vitest 4, zod 4, eslint 10, typescript-eslint 8; `puppeteer-core` 25.12.0 exact; Node `engines` `>=24`; CI `setup-node` 26.
- Connect options:
- BofA capture mode:
- Idle timeout to design around:
- `train inspect` default filter size:
- Download behavior scope:
