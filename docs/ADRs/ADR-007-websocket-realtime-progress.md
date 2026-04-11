# ADR-007: WebSocket for Real-Time Progress Updates

**Status:** Accepted  
**Date:** 2026-04-10  
**Deciders:** Chris Phillipson

---

## Context

Two operations in Finima are long-running and user-facing:

1. **File import** — parsing 10K+ transactions can take 5-15 seconds.
2. **LLM categorization** — batching 20 transactions per call, a 500-transaction import requires 25 LLM calls taking 30-60 seconds total.

Users need real-time progress feedback during these operations. Polling with HTTP requests wastes bandwidth and adds latency (100-200ms per poll round-trip).

## Decision

Establish a single **WebSocket connection** per authenticated session at `WS /api/ws`.

**Protocol:**

- Client authenticates via JWT in the initial WebSocket handshake (query parameter or first message).
- Server pushes JSON event messages:

```json
{ "type": "upload_progress", "upload_id": "uuid", "parsed": 450, "total": 500 }
{ "type": "categorization_progress", "upload_id": "uuid", "categorized": 80, "total": 120, "flagged": 3 }
{ "type": "categorization_complete", "upload_id": "uuid", "total": 120, "flagged": 5 }
{ "type": "recurring_detected", "count": 3 }
{ "type": "flow_detected", "count": 8 }
```

- Client-side `wsStore` (Zustand) receives messages and dispatches updates to relevant UI components.
- Connection auto-reconnects on disconnect with exponential backoff.

**Axum implementation:**

- Use Axum's built-in WebSocket support (`axum::extract::ws::WebSocket`).
- Backend maintains a `HashMap<UserId, Vec<Sender>>` for routing messages to the correct user's connections.
- Backend tasks (import worker, LLM worker) send progress events through a `tokio::sync::broadcast` channel per user.

## Consequences

**Positive:**

- Sub-second progress updates with no polling overhead.
- Single persistent connection for all event types — no per-feature WebSocket.
- Axum's native WebSocket support requires no additional HTTP framework.
- Enables future real-time features (collaborative household editing, live balance updates).

**Negative:**

- WebSocket connections consume a file descriptor per connected user. Acceptable for 1-10 household users.
- Requires heartbeat/ping to detect stale connections. Axum handles this via tower middleware.
- Adds complexity to the frontend (reconnection logic, message routing). Mitigated: encapsulated in `wsStore`.

## Alternatives Considered

1. **HTTP polling** — Simpler to implement but wastes bandwidth and adds 100-200ms latency per update. Poor UX for progress bars. Rejected.
2. **Server-Sent Events (SSE)** — One-directional (server→client), simpler than WebSocket. Viable but WebSocket provides bidirectional capability for future features (e.g., client-initiated cancellation). Rejected.
3. **No real-time updates** — Show a "processing" spinner and redirect when complete. Acceptable but significantly worse UX. Rejected.
