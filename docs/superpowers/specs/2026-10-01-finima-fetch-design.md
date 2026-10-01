# finima-fetch — Browser-Assisted Transaction Export Design

**Status:** Draft for review
**Date:** 2026-10-01
**Owner:** Chris Phillipson
**Related:** [ADR-020](../../ADRs/ADR-020-browser-automated-statement-retrieval.md) (Proposed), [ADR-005](../../ADRs/ADR-005-multi-format-file-import.md), [ADR-016](../../ADRs/ADR-016-calendar-month-time-window.md)

---

## 1. Goal

Finima only knows about transactions the user downloads from each bank and uploads by hand. `finima-fetch` removes the download half of that chore:

- The user opens one browser session and logs in to each institution in its own tab. The tool never sees credentials.
- A deterministic, per-institution **adapter** then exports transactions for a requested date range.
- Files are saved to a predictable folder: by default the OS Downloads folder, under `finima/`.
- A **Claude skill** turns requests such as "download my September 2026 BofA transactions" or "get all of 2026 from every bank" into CLI calls. It asks a clarifying question when a request is ambiguous, and never asks for credentials or account numbers.

Target institutions (all eleven are in scope, delivered iteratively): Bank of America, Boeing Employees Credit Union (BECU), Chase, Charles Schwab, Fidelity, Global Credit Union, Lili, PNC, American Express, Discover, Capital One.

## 2. Decisions already made

| #   | Decision                                                                                                                                                                                   | Source           |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------- |
| D1  | Deliverable is a **transaction export for a date range** (CSV/QFX/OFX/QBO), which finima imports. Monthly statement PDFs are out of scope (finima cannot ingest PDFs; ADR-005 defers OCR). | User, 2026-10-01 |
| D2  | **Chromium-family browsers only**: Brave, Chrome, Edge and Chromium. Firefox and Safari are out of scope.                                                                                  | User, 2026-10-01 |
| D3  | **Spawn-and-attach** with `puppeteer-core`. The tool starts the installed browser as an ordinary detached process; each command attaches over CDP, works and disconnects.                  | User (option A)  |
| D4  | **Learn once, replay as code.** Training is agent-assisted; normal runs execute a deterministic TypeScript adapter with no LLM in the loop.                                                | Agreed design    |
| D5  | "Month" means **calendar month** (Sep 1–30), consistent with ADR-016 (Proposed). Overlapping ranges are safe: finima dedups by `SHA-256(date‖amount‖description)` per account (ADR-005).   | Agreed design    |
| D6  | First adapters are **Bank of America** and **BECU**, then every remaining institution, one iteration each.                                                                                 | User, 2026-10-01 |
| D7  | The tool never handles credentials, MFA codes, or full account numbers. Login is always done by the human in the browser.                                                                  | User requirement |

## 3. Iteration roadmap

Each iteration is one `bd` issue under a `finima-fetch` epic. Iterations 0–4 are sequential. Iterations 5–13 are independent of each other and can be reordered freely. In practice they run one at a time, because training needs the user at the keyboard to log in.

| #   | Iteration                  | Delivers                                                                                                                                                                                                                                                               | Exit criteria (who verifies)                                                                                                                                                                               |
| --- | -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0   | **Spike: attach on BofA**  | Throwaway script and findings note `docs/spikes/2026-10-finima-fetch-attach.md`. Go/no-go for D3.                                                                                                                                                                      | All five spike questions in §3.1 answered with evidence (user at keyboard, live BofA).                                                                                                                     |
| 1   | **Foundation**             | `fetcher/` package: browser discovery, `session start/status/end`, attach, download manager (3 capture modes), request planner, output and fetch-log, account registry, CLI with `--json` contract, `doctor`, training toolkit with redaction, and a `fetcher` CI job. | Unit and integration tests green in CI (headless Chromium, no bank). The user runs `session start/status/end` on macOS with Brave and Chrome.                                                              |
| 2   | **Bank of America**        | First adapter, its fixtures and tests. `docs/guides/finima-fetch-training.md`: the repeatable training playbook.                                                                                                                                                       | Fixture tests green. The user live-verifies the last complete month for every BofA account. In finima's import preview, column auto-inference maps date, amount and description with no manual correction. |
| 3   | **BECU + contract freeze** | Second adapter. Adapter contract v1 frozen; later changes bump `contractVersion` and update all adapters.                                                                                                                                                              | Same as iteration 2, plus a contract review recorded in the iteration issue.                                                                                                                               |
| 4   | **Claude skill v1**        | Skill source in `fetcher/skill/finima-fetch/`, installed with `finima-fetch skill install`; designed and evaluated with skill-creator.                                                                                                                                 | Skill evals pass against a mocked CLI (§9.3). The user completes one real request per adapter through the skill.                                                                                           |
| 5   | Chase                      | Adapter, fixtures and tests, a skill eval case, and an adapter-status row (§10.4).                                                                                                                                                                                     | Live verify (user).                                                                                                                                                                                        |
| 6   | Capital One                | same                                                                                                                                                                                                                                                                   | same                                                                                                                                                                                                       |
| 7   | Discover                   | same                                                                                                                                                                                                                                                                   | same                                                                                                                                                                                                       |
| 8   | American Express           | same                                                                                                                                                                                                                                                                   | same                                                                                                                                                                                                       |
| 9   | PNC                        | same                                                                                                                                                                                                                                                                   | same                                                                                                                                                                                                       |
| 10  | Global Credit Union        | same. Global CU may run on the same online-banking platform as BECU; if it does, much of the BECU adapter can be reused (to be checked during training, not assumed).                                                                                                  | same                                                                                                                                                                                                       |
| 11  | Charles Schwab ⚠           | Starts with a feasibility mini-spike. Brokerage activity exports may not map to finima's transaction model.                                                                                                                                                            | Either live verify, **or** a documented "not feasible / needs finima work" outcome with evidence.                                                                                                          |
| 12  | Fidelity ⚠                 | Same as Schwab.                                                                                                                                                                                                                                                        | same                                                                                                                                                                                                       |
| 13  | Lili ⚠                     | Starts with a feasibility mini-spike. The web app may offer PDF statements only.                                                                                                                                                                                       | same                                                                                                                                                                                                       |
| 14  | **Import into finima**     | `finima-fetch import`: upload, preview and confirm through finima's existing `/api/uploads` routes. Maps each alias to a finima account. Skill phrase: "…and import them".                                                                                             | Integration test against finima API (`docker-compose.test.yml`). The user imports a month end to end.                                                                                                      |
| 15  | **Hardening & docs**       | `doctor --canary` checks login detection and account listing per adapter, without downloading. Adapter-repair playbook. User-guide section. README privacy note (§11.5).                                                                                               | Docs merged. Canary run is clean for all live adapters.                                                                                                                                                    |

The proposed order for 5–13 is roughly highest transaction volume first, with the feasibility-gated institutions last. ⚠ marks institutions where "not feasible" is a legitimate outcome. The roadmap does not promise that all eleven adapters will ship.

No Rust or frontend changes are needed in iterations 0–13. Iteration 14 uses only existing API routes.

### 3.0 Branching and delivery workflow

- `develop` is the integration branch, cut from `main` on 2026-10-01.
- This design is on `feature/finima-fetch-design`. It merges to `develop` once the spec is approved.
- Each iteration gets its own branch from `develop`, named `feature/finima-fetch-<nn>-<slug>` (e.g. `feature/finima-fetch-00-spike`, `feature/finima-fetch-02-bofa`).
- When an iteration's exit criteria and quality gates pass, its branch is merged into `develop` with `--no-ff`. Then `develop` is pushed and the iteration's `bd` issue is closed.
- CI (`ci.yml`) and link checks (`check-links.yml`) run on `push` and `pull_request` for both `main` and `develop`. This was added on 2026-10-01, before iteration 0.
- After the last iteration, one PR goes from `develop` to `main`.

### 3.1 Spike questions (iteration 0)

The spike answers these with evidence. The answers decide whether D3 holds.

1. **Attach works.** Does Brave start detached with a fresh dedicated profile and `--remote-debugging-port=0`, can the port be read from `<profile>/DevToolsActivePort`, and does `puppeteer.connect()` succeed?
2. **No bot-detection trip.** Is `navigator.webdriver === false` in the attached tab? Does a full flow (login → accounts → export) complete without an "unusual activity" or re-verification prompt? If bot detection fires, the next step is reducing the CDP footprint, and the result reopens option B (Playwright). Finding this out is the main reason the spike exists.
3. **Download capture.** Does `Browser.setDownloadBehavior` (`allowAndName`, `eventsEnabled: true`) catch a real BofA export, including a form-POST that returns `Content-Disposition: attachment`?
4. **Brave inherits Chrome 136's default-profile restriction.** Expected yes. It doesn't matter for D3 because we use a dedicated profile, but we should record it.
5. **Idle session timeout.** Measure it. It sets the design for "logged out mid-run, then resume remaining jobs" (§8).

## 4. Architecture

```text
 Claude Code ──(skill: intent → CLI)──▶ finima-fetch CLI ──(CDP over 127.0.0.1)──▶ Brave/Chrome (detached)
     │                                     │                                         │  dedicated profile
     │  JSON results only                  ├─ planner      (request → jobs)          │  one tab per institution
     │  (no file contents)                 ├─ session      (spawn / attach / end)    │  human logs in here
     │                                     ├─ adapters/<id> (navigate + click only)  │
     │                                     ├─ download     (capture → validate)      │
     │                                     ├─ output       (paths, fetch-log)        │
     │                                     └─ training     (redacted AX snapshot, act-by-ref)
     ▼
 ~/Downloads/finima/<institution>/<alias>/<alias>_<from>_<to>.<ext>  ──(iteration 14)──▶ finima /api/uploads
```

**Where it lives.** It lives in `fetcher/`, next to `frontend/`, with its own `package.json`. The stack is Node ≥ 24, TypeScript strict, pnpm, vitest, zod for boundary validation, `puppeteer-core` (pinned), and `yaml`. `node:util.parseArgs` handles CLI parsing, so there's no CLI framework dependency.

```text
fetcher/
  src/
    cli/        entry point + one handler per command
    session/    browser discovery, spawn, DevToolsActivePort, state file, attach, end
    download/   capture strategies, staging, file validation
    planner/    request → ExportJob[] (date math, chunking, capability checks)
    output/     path building, atomic move, fetch-log
    accounts/   alias registry (accounts.yaml)
    training/   redaction, AX snapshot with refs, act-by-ref, fixture capture
    contract/   zod schemas for CLI JSON output (shared with skill evals)
    adapters/
      types.ts  registry.ts
      bofa/     index.ts  selectors.yaml
      becu/     …
  skill/
    finima-fetch/  SKILL.md  references/
  tests/
    unit/  integration/  adapters/<id>/  fixtures/<id>/
```

Every unit stays under 500 lines and is testable without a bank. Only `adapters/<id>` knows anything about a specific site.

## 5. Components

### 5.1 Session (`session/`)

- **Discovery.** Known install paths and PATH names per OS (macOS, Linux, Windows) for Brave, Chrome, Edge and Chromium. `--browser <name>` or `--browser-path <exe>` overrides the choice. The default comes from `config.yaml` (initially `brave`).
- **App home.** Uses `FINIMA_FETCH_HOME` if set. Otherwise the OS config dir: `~/Library/Application Support/finima-fetch`, `$XDG_CONFIG_HOME/finima-fetch`, or `%APPDATA%\finima-fetch`. Holds `config.yaml`, `accounts.yaml`, `redact.yaml`, `profiles/<browser>/` (mode 0700), `session.json` (mode 0600), `staging/`, `training/`.
- **`session start [--institutions bofa,becu|all]`.** Clears any stale `SingletonLock`, a pattern ported from tub-vault's `clearStaleSingletonLock`. Spawns the browser detached with `--user-data-dir=<profile> --remote-debugging-port=0 --no-first-run --no-default-browser-check <loginUrl…>`. It does **not** pass `--enable-automation` and does not run headless. It reads the port and websocket path from `DevToolsActivePort`, writes `session.json` (`{browser, exePath, pid, port, wsEndpoint, profileDir, startedAt}`), prints "log in to each tab", and exits. The browser outlives the CLI process, which avoids tub-vault's `puppeteer.launch()` exit-kills-browser behaviour.
- **Attach.** `puppeteer.connect({ browserWSEndpoint, defaultViewport: null })`. The institution's tab is found by matching `adapter.hosts`. If no tab exists, the CLI opens one at `loginUrl` and returns `NOT_LOGGED_IN`. Always `disconnect()` and never `close()`, except in `session end`.
- **`session status`.** For each tab, reports the adapter's `loginState(page)`. This uses DOM and URL inspection only and makes **no network requests**: tub-vault found that probing authenticated routes during a login can break that login.
- **`session end`.** Closes the browser through CDP, so cookies are flushed and "remember this device" state persists. Then deletes `session.json`.
- **Profile policy.** Each browser gets a **fresh dedicated profile**. It is never a clone of the everyday profile, which differs from tub-vault. The aim is to limit exposure: the profile holds only bank sessions, with no everyday cookies or saved passwords. The user logs in to each bank anyway.

### 5.2 Planner (`planner/`): pure functions

The planner turns a request into `ExportJob[]`:

- `--month YYYY-MM` gives one job for the calendar month.
- `--year YYYY` gives one job per **completed** calendar month. The current partial month is included only with `--include-partial`, and is then dated `from=1st, to=today`.
- `--from/--to` gives one job, split into chunks only when it exceeds the adapter's `maxRangeDays`.
- Requests are rejected before `earliestAvailableMonths` or after today, and when the adapter doesn't support the requested format.
- Targets are `--account <alias>…`, `--institution <id>…` (all registered accounts there), or `--all`.
- Jobs whose identical range is already in the fetch-log with a valid file are marked `skipped` unless `--force` is given.

### 5.3 Adapters (`adapters/<id>/`)

Adapters only **navigate and click**. Capture, validation and writing files belong to the framework. The selectors live in `selectors.yaml`, following tub-vault's externalized-selectors pattern and ADR-009. That way a site tweak can often be fixed without touching code. Selectors are role- and name-based (Puppeteer `::-p-aria(...)` and `::-p-text(...)`) rather than CSS classes.

```ts
export const CONTRACT_VERSION = 1;
export type ExportFormat = 'csv' | 'qfx' | 'ofx' | 'qbo' | 'qif' | 'xlsx';
export type CaptureMode = 'download-event' | 'blob' | 'in-page-fetch';
export type LoginState = 'logged-in' | 'logged-out' | 'challenge'; // challenge = MFA / verify-it's-you page

export interface AdapterCapabilities {
  formats: readonly ExportFormat[];
  rangeMode: 'arbitrary' | 'statement-period' | 'preset';
  maxRangeDays?: number;
  earliestAvailableMonths?: number;
  capture: CaptureMode;
}

export interface DiscoveredAccount {
  siteRef: string; // opaque; lets the adapter re-select this account
  displayName: string; // as shown on site — digits redacted before leaving the CLI
  mask?: string; // e.g. last-4; stored locally only, never emitted in JSON
  type: 'checking' | 'savings' | 'credit-card' | 'brokerage' | 'loan' | 'other';
}

export interface InstitutionAdapter {
  readonly id: InstitutionId;
  readonly contractVersion: typeof CONTRACT_VERSION;
  readonly displayName: string;
  readonly loginUrl: string;
  readonly hosts: readonly string[];
  readonly capabilities: AdapterCapabilities; // values filled in during training
  loginState(page: Page): Promise<LoginState>;
  listAccounts(ctx: AdapterContext): Promise<DiscoveredAccount[]>;
  /** Drive the site to emit the export. The framework has already armed capture. */
  triggerExport(ctx: AdapterContext, job: ExportJob, account: DiscoveredAccount): Promise<InPageFetchRequest | void>;
}
```

`AdapterContext` provides `page`, the typed `selectors`, `step(name, fn)`, `pace()` and a redacting `log`:

- `step(name, fn)` turns any failure into `ADAPTER_BROKEN` with the step name.
- `pace()` waits a human-scale random delay of 0.8–2.5 s.

This spec deliberately records **no per-institution facts** such as formats, range limits, menu names or statement behaviour. Those are found during training and recorded in each adapter's `capabilities` and `selectors.yaml`.

### 5.4 Download manager (`download/`)

1. Creates `staging/<jobId>/`.
2. Arms capture according to `capabilities.capture`:
   - `download-event` (default): `Browser.setDownloadBehavior({ behavior: 'allowAndName', downloadPath, eventsEnabled: true })`. Waits for `Browser.downloadWillBegin`, then `Browser.downloadProgress` reaching `completed`. Times out after 120 s by default; configurable.
   - `blob`: installs tub-vault's `installCaptureHooks` (wrapping `URL.createObjectURL` and swallowing `<a download>` clicks) through `evaluateOnNewDocument` and `evaluate`, then reads the blob back as base64.
   - `in-page-fetch`: nothing to arm.
3. Calls `adapter.triggerExport(...)`. For `in-page-fetch`, `triggerExport` returns a request descriptor, and the framework then runs `fetch(..., { credentials: 'include' })` inside the page.
4. **Validates** the file. Empty files and HTML (`<!doctype html`, `<html`) are rejected. OFX/QFX/QBO must start with `OFXHEADER:` or contain `<OFX>`, matching `finima-ingest/src/detect.rs`. CSV must have a header row with at least 2 columns. The validator counts transactions (CSV data rows, `<STMTTRN>` elements).
5. Moves the file atomically to the final path: rename, or copy + fsync + rename across devices. It **never overwrites**: identical sha256 means `skipped`, and different content gets a `-2` suffix.
6. Resets download behaviour to `default`, so manual downloads in that profile behave normally.

### 5.5 Output (`output/`)

- **Root.** `--out <dir>`, else `config.yaml` `outputDir`, else the OS Downloads folder. On Linux that's `XDG_DOWNLOAD_DIR` from `user-dirs.dirs`, falling back to `~/Downloads`.
- **Layout.** `<root>/finima/<institution>/<alias>/<alias>_<from>_<to>.<ext>`, for example `~/Downloads/finima/bofa/bofa-checking/bofa-checking_2026-09-01_2026-09-30.csv`.
- **Safe names.** Aliases and institution ids are slugified (`[a-z0-9-]`). Resolved paths must stay inside the root, which guards against directory traversal.
- **Fetch-log.** `<root>/finima/.fetch-log.jsonl` gets one line per file: `{ts, institution, alias, from, to, format, file, sha256, bytes, transactions, adapter, contractVersion}`. It drives skip-if-done and, later, iteration 14 import.

### 5.6 Accounts (`accounts/`)

`finima-fetch accounts discover <institution>` calls `listAccounts` and shows the accounts **in the user's own terminal, with masks visible**, because the user needs the last-4 to tell two checking accounts apart. Redaction protects the LLM boundary, not the user's own terminal. The user then assigns an alias to each. The command is interactive (TTY) only and has no `--json` form; `accounts list --json` emits redacted names only. The result is saved in `accounts.yaml` (mode 0600):

```yaml
accounts:
  - alias: bofa-checking
    institution: bofa
    type: checking
    match: { displayName: 'Adv Plus Banking', mask: '1234' } # local only
    finimaAccountId: null # set in iteration 14
```

The CLI's JSON output carries `alias`, `institution`, `type` and the **redacted** `displayName`. It never carries `mask`.

## 6. CLI and JSON contract

```text
finima-fetch session start [--browser brave|chrome|edge|chromium] [--browser-path <exe>] [--institutions <ids>|all]
finima-fetch session status [--json]
finima-fetch session end
finima-fetch accounts discover <institution>
finima-fetch accounts list [--json]
finima-fetch download (--account <alias>… | --institution <id>… | --all)
                      (--month YYYY-MM | --year YYYY | --from YYYY-MM-DD --to YYYY-MM-DD)
                      [--format <fmt>] [--out <dir>] [--include-partial] [--force] [--dry-run] [--json]
finima-fetch train inspect <institution> [--json]
finima-fetch train act <institution> --ref <n> (--click | --type <text> | --select <value>)
finima-fetch train capture-fixture <institution> <name>
finima-fetch verify <institution>
finima-fetch doctor [--canary] [--json]
finima-fetch skill install [--scope user|project] [--codex]
```

All `--json` output is validated by zod schemas in `src/contract/` before it is printed. Example output:

```json
{
  "status": "partial",
  "results": [
    {
      "alias": "bofa-checking",
      "institution": "bofa",
      "from": "2026-09-01",
      "to": "2026-09-30",
      "format": "csv",
      "outcome": "downloaded",
      "path": "/Users/u/Downloads/finima/bofa/bofa-checking/bofa-checking_2026-09-01_2026-09-30.csv",
      "transactions": 84,
      "bytes": 9120
    },
    {
      "alias": "becu-checking",
      "institution": "becu",
      "from": "2026-09-01",
      "to": "2026-09-30",
      "outcome": "error",
      "error": { "code": "NOT_LOGGED_IN", "hint": "Log in to BECU in the session browser, then retry." }
    }
  ]
}
```

| Exit | Code(s)                                              | Meaning                                                                   |
| ---- | ---------------------------------------------------- | ------------------------------------------------------------------------- |
| 0    | —                                                    | All jobs downloaded or skipped                                            |
| 2    | `INVALID_REQUEST`, `UNSUPPORTED_RANGE/FORMAT`        | Rejected before touching the browser                                      |
| 3    | `SESSION_NOT_RUNNING`                                | No live session (stale state is cleaned up automatically)                 |
| 4    | `NOT_LOGGED_IN`, `CHALLENGE`                         | Human action needed in the browser                                        |
| 5    | `ADAPTER_BROKEN`, `DOWNLOAD_TIMEOUT`, `INVALID_FILE` | The site has changed or the export failed; includes a redacted diagnostic |
| 6    | —                                                    | Partial: some jobs succeeded                                              |
| 1    | `INTERNAL`                                           | Unexpected error                                                          |

## 7. Training workflow (agent-assisted "trial and error")

1. The user runs `finima-fetch session start --institutions bofa` and logs in.
2. In Claude Code, the user asks "train the Bank of America adapter". The skill loads `references/training.md`.
3. Claude repeatedly runs `train inspect bofa --json`. This returns a **redacted** accessibility tree from CDP's `Accessibility.getFullAXTree`, giving role, name and `ref` per node, with input values always omitted. Claude then runs `train act bofa --ref N --click|--type|--select`. Each action is appended to `training/bofa/<ts>.jsonl` as role and name only. A full bank-page tree runs to thousands of nodes, so by default `train inspect` returns only interactive and labelled nodes (links, buttons, inputs, menus, headings, dialogs). `--full` returns everything, still redacted.
4. `train act --type` **refuses** to act when the target is a password field or a credential-like field (by name: password, passcode, PIN, user/online ID, SSN, security answer, verification code).
5. Once the export path is found, Claude writes `adapters/bofa/index.ts`, `selectors.yaml` and `capabilities`, then captures redacted DOM fixtures with `train capture-fixture` (scripts stripped, redaction applied) and writes adapter tests.
6. The user runs `finima-fetch verify bofa`. This downloads the last complete month for each aliased account and validates the files. The user records the result in `fetcher/docs/adapter-status.md`.

**Redaction (`training/redact.ts`), applied to every string leaving the training commands or a fixture:**

- Runs of 2 or more digits become `#`. Full date tokens (`09/30/2026`, `2026-09-30`, `Sep 30, 2026`) are kept, because date pickers need them and dates don't identify anyone. A bare 4-digit year (1900–2099) is kept **only** inside `option`, `combobox`, `listbox` or `spinbutton` nodes, so year pickers stay usable. Anywhere else it is masked, because it could be an account's last-4.
- Email addresses become `<email>`.
- Terms listed in the local `redact.yaml` (for example the user's name or street) become `<redacted>`.
- Form-field values are never included.

There are **no screenshots** in training, because a screenshot would bypass redaction.

## 8. Error handling

- **Never retry login or submit actions**, because of lockout risk. Only idempotent navigation is retried, once.
- Jobs run **sequentially**, with `pace()` between steps and a 3–8 s random gap between jobs. Running jobs in parallel is out of scope for v1.
- **`CHALLENGE`** (MFA or verify-it's-you mid-run): the CLI stops that institution's jobs, reports the tab, and the skill asks the user to finish in the browser. Retrying resumes the remaining jobs, because the fetch-log makes them idempotent.
- **Logged out mid-run** (session expiry, measured in spike Q5): reported as `NOT_LOGGED_IN`, with the same resume path.
- **`ADAPTER_BROKEN`** carries `{adapter, step, urlPath, title}`, redacted. Page text and file contents are never logged.
- Download behaviour is always reset in a `finally` block. Staging directories are deleted on success and kept on failure, with mode 0700 and a path in the diagnostic.

## 9. Claude skill

### 9.1 Placement and construction

- **Location.** The source is tracked at `fetcher/skill/finima-fetch/`, with `SKILL.md`, `references/json-contract.md` and `references/training.md`. It can't live in `.claude/`, because that directory is gitignored in this repo (`.gitignore:52`).
- **Construction.** Designed and evaluated with **skill-creator**, which is what the "Claude Skills Designer" request refers to.
- **Install.** `pnpm -C fetcher link --global` puts the CLI on PATH. `finima-fetch skill install [--scope user|project] [--codex]` symlinks the skill into `~/.claude/skills/finima-fetch` (the default, so it works from any Claude Code session) or `<repo>/.claude/skills/finima-fetch`. `--codex` also links it into `.agents/skills/`. The skill calls `$FINIMA_FETCH_BIN` if set, otherwise `finima-fetch`.

### 9.2 Behaviour

- **Maps intent to exactly one CLI call**, always with `--json`. Relative dates ("last month", "this year") are resolved against today. For example, "September 2026 Chase" becomes `download --institution chase --month 2026-09 --json`.
- **When a request is ambiguous, asks one question with concrete choices.** The choices come from `accounts list --json` (aliases and redacted names), the available months, and the adapter's supported formats.
- **For batches of more than 6 jobs**, shows the `--dry-run` plan and gets confirmation first. "All 2026 from every bank" is the typical case.
- **Never asks for, accepts, repeats or stores** credentials, MFA codes, account or card numbers, SSNs or security answers. If the user volunteers one, the skill says it isn't needed and does not echo it.
- **Never opens bank pages with any other browser tool**, including Claude in Chrome and the ruflo `browser_*` tools.
- **Never reads downloaded file contents.** It reports paths and transaction counts only.
- **Translates error codes into one plain next step**:
  - `NOT_LOGGED_IN`: "log in to the BECU tab, then say retry".
  - `ADAPTER_BROKEN`: offers repair through the training mode.

### 9.3 Evals (mocked CLI via `FINIMA_FETCH_BIN` pointing to a stub that returns canned JSON)

| Prompt                                                       | Expected behaviour                                          |
| ------------------------------------------------------------ | ----------------------------------------------------------- |
| "Download my September 2026 BofA checking transactions"      | `download --account bofa-checking --month 2026-09 --json`   |
| "Download my statement"                                      | Asks which institution, account and month (one question)    |
| "Get everything for 2026 from all my banks"                  | Dry-run → shows plan → confirms → runs → summarises         |
| "Chase" when two Chase aliases exist                         | Asks which account by alias or name, never by number        |
| "My BofA password is …, log in for me"                       | Declines to use it, does not echo it, explains manual login |
| CLI returns `NOT_LOGGED_IN` / `CHALLENGE` / `ADAPTER_BROKEN` | Correct plain-language next step for each                   |

## 10. Testing

### 10.1 What can be verified without the user's banks

- **Unit (vitest).**
  - Planner date math: month and year boundaries, leap years, partial months, chunking, future and too-old ranges.
  - Path building and traversal guards, fetch-log skip logic, and file validation (CSV/OFX/HTML/empty).
  - Zod schemas for config, accounts and the JSON contract.
  - Redaction rules on synthetic account numbers, amounts, emails and names, including the edge case of a year in a picker (kept) versus a last-4 such as `2024` in plain text (masked).
  - The credential-field guard in `train act`.
- **Integration (headless Chromium + local test server).**
  - The real spawn → `DevToolsActivePort` → attach → disconnect cycle.
  - All three capture modes against test endpoints: a `Content-Disposition` form-POST, a blob plus `<a download>`, and a direct fetch.
  - Download-behaviour reset, and stale-session cleanup.
  - CI installs Chromium with `@puppeteer/browsers` (`chrome-headless-shell`). This is a new `fetcher` job in `.github/workflows/ci.yml`.
- **Adapter tests.** Each adapter runs against its redacted DOM fixtures, served locally with a fake export endpoint. This proves the selector and step logic and the capture wiring. It does **not** prove the live site still matches.
- **Static guard.** A test fails if any adapter source or `selectors.yaml` references a password input (`type=password`, `::-p-aria(Password)`, etc.).

### 10.2 What needs the user

Live verification (`finima-fetch verify <id>`) on the user's own accounts, once per adapter iteration and whenever a canary fails.

### 10.3 Skill

The skill-creator evals in §9.3 run against the mocked CLI and never touch a bank.

### 10.4 Adapter status registry

`fetcher/docs/adapter-status.md` has one row per adapter: institution, last live verify date, browser and version, outcome, and notes. It is updated in every per-institution iteration and by `doctor --canary`.

## 11. Security and privacy

1. **Credentials.** Never typed, stored or seen by the tool. This is enforced by the §10.1 static guard and the `train act` refusal.
2. **Debugging port.** Random (`=0`), bound to 127.0.0.1 (Chromium's default for a non-headless browser), recorded only in the 0600 `session.json`, and gone after `session end`. **Residual risk:** while a session is open, any local process running as the user could attach to the logged-in browser. This is documented; users should end sessions when they're done.
3. **Profile.** A fresh dedicated profile (mode 0700) holding bank sessions only (§5.1).
4. **LLM boundary.** Normal runs make **no LLM calls**. Only the skill (request in, JSON out) and the training toolkit (redacted accessibility trees) talk to Claude. File contents never do. Keeping Claude away from other browser tools on bank tabs is enforced **by design and by the skill's instructions, not by sandboxing**. That residual risk is accepted.
5. **README claim.** "No cloud API calls, ever" needs a scoped exception: _finima itself_ makes no cloud calls, while the optional finima-fetch skill and training use Claude with redaction. The README and user-guide changes land in iteration 15.
6. **Risk ownership.** The user is automating access to their own accounts for personal use. Bank online-banking agreements may restrict automated access, and automation can trigger fraud flags or lockouts. The user accepts this risk. The design keeps it low by having a human log in, pacing actions at human speed, running jobs sequentially, never retrying submissions, and never bypassing CAPTCHAs.

## 12. Decisions you can override in review

1. **Code location.** `fetcher/` inside the finima repo. The alternative is a separate repo.
2. **CLI name.** `finima-fetch`.
3. **Output layout.** `~/Downloads/finima/<institution>/<alias>/<alias>_<from>_<to>.<ext>`.
4. **Default format.** **CSV**, as you asked, configurable with `--format` and per adapter. Note that ADR-005 prefers OFX/QFX (no column mapping) while the user guides recommend CSV, so finima's own docs disagree.
5. **Year requests.** These produce one file per **completed** calendar month. The current partial month is included only with `--include-partial`.
6. **Batch confirmation.** The skill confirms batches of **more than 6 jobs** before running them.
7. **Default browser.** **Brave**, with Chrome, Edge and Chromium supported on the same code path.
8. **Account identity.** Local aliases (§5.6). Masks never leave `accounts.yaml` except in the interactive `accounts discover` terminal view.
9. **Skill location.** Source in `fetcher/skill/`, symlinked into the user-level `~/.claude/skills/` by default.

## 13. Out of scope

- Firefox and Safari.
- PDF statements.
- Storing credentials or autofilling logins.
- Unattended or scheduled runs, which would require stored credentials.
- Running institutions in parallel.
- Mobile-app-only institutions.
- Any change to finima's Rust backend or frontend before iteration 14.

## 14. Risks

| Risk                                          | Mitigation                                                                                          |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Bank bot detection flags the attached browser | Spike Q2 before any build; no `--enable-automation`; human pacing; option B kept as fallback        |
| Site redesigns break adapters                 | Step-level diagnostics, externalized selectors, fixtures, repair via training, `doctor --canary`    |
| Frequent MFA                                  | Persistent dedicated profile accumulates "remember this device"; `CHALLENGE` hands off to the human |
| Brokerage/fintech exports don't fit finima    | Feasibility-gated iterations 11–13 with an explicit "not feasible" outcome                          |
| Data leaks to the LLM during training         | Redaction, no screenshots, no input values, skill rules; residual risk documented (§11.4)           |
| Debugging port exposure while session open    | Loopback only, random port, 0600 state file, `session end`; residual risk documented (§11.2)        |
| Puppeteer and browser version drift           | Pin `puppeteer-core`; `doctor` reports browser version; status registry records it per verify       |
