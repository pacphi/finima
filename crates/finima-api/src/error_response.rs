// This module exists to document the architectural boundary for error handling.
//
// The `IntoResponse` implementation for `AppError` lives in `finima-core` but
// is gated behind the optional `axum` feature flag.  Only crates that need
// HTTP response conversion (like this API crate) enable the feature via:
//
//     finima-core = { path = "../finima-core", features = ["axum"] }
//
// Crates that use `AppError` purely for domain logic (CLI tools, background
// workers, etc.) can depend on `finima-core` without pulling in `axum` at all.
