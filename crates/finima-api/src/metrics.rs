//! Prometheus metrics for the Finima API server.
//!
//! Uses the `prometheus-client` crate (OpenMetrics-compatible) to expose
//! SRE-essential request metrics, database pool metrics, business metrics,
//! and error budget indicators.

use std::sync::Arc;

use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::{exponential_buckets, Histogram};
use prometheus_client::registry::Registry;

// ---------------------------------------------------------------------------
// Label sets
// ---------------------------------------------------------------------------

/// Labels for HTTP request metrics: method, path template, and status code.
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct HttpRequestLabels {
    pub method: String,
    pub path: String,
    pub status_code: u16,
}

/// Labels for HTTP duration metrics (no status_code — recorded before response
/// status is known at histogram-observe time, but we include it for consistency
/// so latency can be sliced by endpoint).
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct HttpDurationLabels {
    pub method: String,
    pub path: String,
}

/// Labels for counters that track success/failure outcomes.
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct ResultLabels {
    pub result: ResultKind,
}

/// Outcome discriminant shared across several business metrics.
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelValue)]
#[allow(dead_code)]
pub enum ResultKind {
    Success,
    Failure,
    Timeout,
}

/// Labels for upload counters.
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct UploadStatusLabels {
    pub status: UploadStatus,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelValue)]
#[allow(dead_code)]
pub enum UploadStatus {
    Success,
    Failure,
}

/// Labels for Tier 2 query counters: backend ("jaccard" | "ruvector") and outcome
/// ("matched" | "rejected" | "error").
#[derive(Clone, Hash, PartialEq, Eq, EncodeLabelSet, Debug)]
pub struct Tier2QueryLabels {
    pub backend: String,
    pub outcome: String,
}

/// Labels for Tier 2 latency histograms: backend only.
#[derive(Clone, Hash, PartialEq, Eq, EncodeLabelSet, Debug)]
pub struct Tier2BackendLabel {
    pub backend: String,
}

/// Labels for flow-pattern matcher query counters: backend ("stub" | "ruvector")
/// and outcome ("resolved" | "miss" | "error").
#[derive(Clone, Hash, PartialEq, Eq, EncodeLabelSet, Debug)]
pub struct FlowQueryLabels {
    pub backend: String,
    pub outcome: String,
}

/// Labels for flow-pattern matcher latency histograms: backend only.
#[derive(Clone, Hash, PartialEq, Eq, EncodeLabelSet, Debug)]
pub struct FlowBackendLabel {
    pub backend: String,
}

/// Labels for bootstrap duration histograms: component ("tier2" | "flows") and
/// result ("ok" | "partial" | "failed").
#[derive(Clone, Hash, PartialEq, Eq, EncodeLabelSet, Debug)]
pub struct BootstrapLabels {
    pub component: String,
    pub result: String,
}

// ---------------------------------------------------------------------------
// Metrics struct
// ---------------------------------------------------------------------------

/// All application metrics collected in one place.
///
/// Each field is a prometheus-client metric that has already been registered
/// with the inner `Registry`. Clone is cheap (inner state is `Arc`-wrapped by
/// the prometheus-client metric types).
#[allow(dead_code)]
#[derive(Clone)]
pub struct Metrics {
    // -- HTTP / SRE essentials ------------------------------------------------
    /// Total HTTP requests by method, path, status_code.
    pub http_requests_total: Family<HttpRequestLabels, Counter>,
    /// HTTP request duration histogram by method, path.
    pub http_request_duration_seconds: Family<HttpDurationLabels, Histogram>,
    /// Number of HTTP requests currently being processed.
    pub http_requests_in_flight: Gauge,

    // -- Database pool --------------------------------------------------------
    /// Active connections in the database pool.
    pub db_pool_connections_active: Gauge,
    /// Idle connections in the database pool.
    pub db_pool_connections_idle: Gauge,
    /// Database query duration histogram.
    pub db_query_duration_seconds: Histogram,

    // -- Business: uploads ----------------------------------------------------
    /// Upload attempts by status (success/failure).
    pub uploads_total: Family<UploadStatusLabels, Counter>,
    /// Total bytes processed across all uploads.
    pub uploads_bytes_total: Counter,

    // -- Business: transactions -----------------------------------------------
    /// Number of transactions imported.
    pub transactions_imported_total: Counter,

    // -- Business: LLM categorization -----------------------------------------
    /// LLM categorization latency histogram.
    pub llm_categorization_duration_seconds: Histogram,
    /// LLM categorization attempts by result.
    pub llm_categorization_total: Family<ResultLabels, Counter>,

    // -- Business: WebSocket --------------------------------------------------
    /// Currently active WebSocket connections.
    pub websocket_connections_active: Gauge,

    // -- Business: auth -------------------------------------------------------
    /// Magic links sent.
    pub magic_links_sent_total: Counter,
    /// Auth token refresh attempts by result.
    pub auth_refresh_total: Family<ResultLabels, Counter>,

    // -- Error budget ---------------------------------------------------------
    /// 5xx error count (for computing error budget burn rate).
    pub error_rate_5xx: Counter,

    // -- Tier 2 categorization ------------------------------------------------
    /// Total Tier 2 semantic categorization queries by backend and outcome.
    pub tier2_queries_total: Family<Tier2QueryLabels, Counter>,
    /// Tier 2 search latency histogram by backend.
    pub tier2_search_latency_seconds: Family<Tier2BackendLabel, Histogram>,
    /// Labeled examples inserted during Tier 2 bootstrap.
    pub tier2_bootstrap_inserted_total: Counter,
    /// Labeled examples rejected during Tier 2 bootstrap.
    pub tier2_bootstrap_rejected_total: Counter,
    /// Tier 2 store size in entries, as observed once at process boot.
    ///
    /// CAVEAT: this does **not** update at runtime. The live server's
    /// request path is query-only for Tier 2 — there is no `.learn()` /
    /// `.learn_with_vector()` call site anywhere in it — so this gauge is
    /// set exactly once, from the freshly-constructed (and therefore
    /// empty) in-memory store, before the router builds or the listener
    /// binds (see `AppState::set_metrics`). It will read 0 for the
    /// lifetime of every real deployment. The `bootstrap_tier2` binary
    /// that populates a Tier 2 store runs as a separate, throwaway-store
    /// process and cannot mutate this gauge. Do not build dashboards or
    /// alerts that expect this value to grow.
    pub tier2_index_size: Gauge<i64, std::sync::atomic::AtomicI64>,

    // -- Flow-pattern matcher (ADR-017) ---------------------------------------
    /// Total flow-pattern matcher queries by backend and outcome.
    pub flow_pattern_queries_total: Family<FlowQueryLabels, Counter>,
    /// Flow-pattern matcher search latency histogram by backend.
    pub flow_pattern_search_latency_seconds: Family<FlowBackendLabel, Histogram>,
    /// Flow patterns confirmed and persisted.
    pub flow_patterns_confirmed_total: Counter,
    /// Flow patterns dismissed.
    pub flow_patterns_dismissed_total: Counter,
    /// Current flow-pattern store size in entries.
    pub flow_pattern_index_size: Gauge<i64, std::sync::atomic::AtomicI64>,

    // -- Bootstrap (Tier 2 + Flow) --------------------------------------------
    /// Bootstrap task duration in seconds by component and result.
    pub bootstrap_duration_seconds: Family<BootstrapLabels, Histogram>,
}

// ---------------------------------------------------------------------------
// MetricsRegistry
// ---------------------------------------------------------------------------

/// Thread-safe wrapper around the prometheus-client `Registry` and the
/// pre-registered `Metrics` struct.
///
/// `Clone` is cheap: the inner data lives behind an `Arc`.
#[derive(Clone)]
pub struct MetricsRegistry {
    inner: Arc<MetricsRegistryInner>,
}

struct MetricsRegistryInner {
    registry: Registry,
    metrics: Metrics,
}

impl MetricsRegistry {
    /// Create a new registry, register all metrics, and return the wrapper.
    pub fn new() -> Self {
        let mut registry = Registry::default();

        // -- HTTP request metrics ---------------------------------------------
        let http_requests_total = Family::<HttpRequestLabels, Counter>::default();
        registry.register(
            "http_requests_total",
            "Total number of HTTP requests",
            http_requests_total.clone(),
        );

        let http_request_duration_seconds =
            Family::<HttpDurationLabels, Histogram>::new_with_constructor(|| {
                // Buckets: 5ms, 10ms, 25ms, 50ms, 100ms, 250ms, 500ms, 1s, 2.5s, 5s, 10s
                Histogram::new(exponential_buckets(0.005, 2.0, 11))
            });
        registry.register(
            "http_request_duration_seconds",
            "HTTP request duration in seconds",
            http_request_duration_seconds.clone(),
        );

        let http_requests_in_flight = Gauge::default();
        registry.register(
            "http_requests_in_flight",
            "Number of HTTP requests currently being processed",
            http_requests_in_flight.clone(),
        );

        // -- Database pool metrics --------------------------------------------
        let db_pool_connections_active = Gauge::default();
        registry.register(
            "db_pool_connections_active",
            "Number of active database pool connections",
            db_pool_connections_active.clone(),
        );

        let db_pool_connections_idle = Gauge::default();
        registry.register(
            "db_pool_connections_idle",
            "Number of idle database pool connections",
            db_pool_connections_idle.clone(),
        );

        let db_query_duration_seconds = Histogram::new(exponential_buckets(0.001, 2.0, 14));
        registry.register(
            "db_query_duration_seconds",
            "Database query duration in seconds",
            db_query_duration_seconds.clone(),
        );

        // -- Upload metrics ---------------------------------------------------
        let uploads_total = Family::<UploadStatusLabels, Counter>::default();
        registry.register(
            "uploads_total",
            "Total upload attempts by status",
            uploads_total.clone(),
        );

        let uploads_bytes_total = Counter::default();
        registry.register(
            "uploads_bytes_total",
            "Total bytes processed across all uploads",
            uploads_bytes_total.clone(),
        );

        // -- Transaction metrics ----------------------------------------------
        let transactions_imported_total = Counter::default();
        registry.register(
            "transactions_imported_total",
            "Total number of transactions imported",
            transactions_imported_total.clone(),
        );

        // -- LLM categorization metrics ---------------------------------------
        let llm_categorization_duration_seconds =
            Histogram::new(exponential_buckets(0.01, 2.0, 12));
        registry.register(
            "llm_categorization_duration_seconds",
            "LLM categorization latency in seconds",
            llm_categorization_duration_seconds.clone(),
        );

        let llm_categorization_total = Family::<ResultLabels, Counter>::default();
        registry.register(
            "llm_categorization_total",
            "Total LLM categorization attempts by result",
            llm_categorization_total.clone(),
        );

        // -- WebSocket metrics ------------------------------------------------
        let websocket_connections_active = Gauge::default();
        registry.register(
            "websocket_connections_active",
            "Number of active WebSocket connections",
            websocket_connections_active.clone(),
        );

        // -- Auth metrics -----------------------------------------------------
        let magic_links_sent_total = Counter::default();
        registry.register(
            "magic_links_sent_total",
            "Total magic links sent",
            magic_links_sent_total.clone(),
        );

        let auth_refresh_total = Family::<ResultLabels, Counter>::default();
        registry.register(
            "auth_refresh_total",
            "Auth token refresh attempts by result",
            auth_refresh_total.clone(),
        );

        // -- Error budget metrics ---------------------------------------------
        let error_rate_5xx = Counter::default();
        registry.register(
            "error_rate_5xx",
            "Total 5xx errors for error budget burn rate calculation",
            error_rate_5xx.clone(),
        );

        // -- Tier 2 categorization metrics ------------------------------------
        let tier2_queries_total = Family::<Tier2QueryLabels, Counter>::default();
        registry.register(
            "tier2_queries_total",
            "Total Tier 2 semantic categorization queries, labeled by backend and outcome.",
            tier2_queries_total.clone(),
        );

        let tier2_search_latency_seconds =
            Family::<Tier2BackendLabel, Histogram>::new_with_constructor(|| {
                // 100 µs to ~3 s
                Histogram::new(exponential_buckets(0.0001, 2.0, 16))
            });
        registry.register(
            "tier2_search_latency_seconds",
            "Latency of Tier 2 k-NN / similarity search in seconds, labeled by backend.",
            tier2_search_latency_seconds.clone(),
        );

        let tier2_bootstrap_inserted_total = Counter::default();
        registry.register(
            "tier2_bootstrap_inserted_total",
            "Labeled examples inserted during Tier 2 bootstrap.",
            tier2_bootstrap_inserted_total.clone(),
        );

        let tier2_bootstrap_rejected_total = Counter::default();
        registry.register(
            "tier2_bootstrap_rejected_total",
            "Labeled examples rejected during Tier 2 bootstrap.",
            tier2_bootstrap_rejected_total.clone(),
        );

        let tier2_index_size = Gauge::<i64, std::sync::atomic::AtomicI64>::default();
        registry.register(
            "tier2_index_size",
            "Tier 2 store size in entries, set once at boot from the (always-empty) \
             freshly-constructed store. Does NOT update at runtime — Tier 2 has no \
             live-mutation path in the request-serving process. Do not alert on growth.",
            tier2_index_size.clone(),
        );

        // -- Flow-pattern matcher metrics -------------------------------------
        let flow_pattern_queries_total = Family::<FlowQueryLabels, Counter>::default();
        registry.register(
            "flow_pattern_queries_total",
            "Total flow-pattern matcher queries, labeled by backend and outcome.",
            flow_pattern_queries_total.clone(),
        );

        let flow_pattern_search_latency_seconds =
            Family::<FlowBackendLabel, Histogram>::new_with_constructor(|| {
                Histogram::new(exponential_buckets(0.0001, 2.0, 16))
            });
        registry.register(
            "flow_pattern_search_latency_seconds",
            "Latency of flow-pattern matcher search in seconds, labeled by backend.",
            flow_pattern_search_latency_seconds.clone(),
        );

        let flow_patterns_confirmed_total = Counter::default();
        registry.register(
            "flow_patterns_confirmed_total",
            "Flow patterns confirmed and persisted.",
            flow_patterns_confirmed_total.clone(),
        );

        let flow_patterns_dismissed_total = Counter::default();
        registry.register(
            "flow_patterns_dismissed_total",
            "Flow patterns dismissed.",
            flow_patterns_dismissed_total.clone(),
        );

        let flow_pattern_index_size = Gauge::<i64, std::sync::atomic::AtomicI64>::default();
        registry.register(
            "flow_pattern_index_size",
            "Current flow-pattern store size in entries.",
            flow_pattern_index_size.clone(),
        );

        // -- Bootstrap metrics ------------------------------------------------
        let bootstrap_duration_seconds =
            Family::<BootstrapLabels, Histogram>::new_with_constructor(|| {
                // 10 ms to ~26 min
                Histogram::new(exponential_buckets(0.01, 2.0, 18))
            });
        registry.register(
            "bootstrap_duration_seconds",
            "Bootstrap task duration in seconds, labeled by component and result.",
            bootstrap_duration_seconds.clone(),
        );

        let metrics = Metrics {
            http_requests_total,
            http_request_duration_seconds,
            http_requests_in_flight,
            db_pool_connections_active,
            db_pool_connections_idle,
            db_query_duration_seconds,
            uploads_total,
            uploads_bytes_total,
            transactions_imported_total,
            llm_categorization_duration_seconds,
            llm_categorization_total,
            websocket_connections_active,
            magic_links_sent_total,
            auth_refresh_total,
            error_rate_5xx,
            tier2_queries_total,
            tier2_search_latency_seconds,
            tier2_bootstrap_inserted_total,
            tier2_bootstrap_rejected_total,
            tier2_index_size,
            flow_pattern_queries_total,
            flow_pattern_search_latency_seconds,
            flow_patterns_confirmed_total,
            flow_patterns_dismissed_total,
            flow_pattern_index_size,
            bootstrap_duration_seconds,
        };

        Self {
            inner: Arc::new(MetricsRegistryInner { registry, metrics }),
        }
    }

    /// Return a reference to the pre-registered metrics for recording.
    pub fn metrics(&self) -> &Metrics {
        &self.inner.metrics
    }

    /// Encode all registered metrics in Prometheus text exposition format.
    pub fn render(&self) -> String {
        let mut buf = String::new();
        prometheus_client::encoding::text::encode(&mut buf, &self.inner.registry)
            .expect("writing to a String never fails");
        buf
    }
}
