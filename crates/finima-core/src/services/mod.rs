//! Domain services that compose pure-data types into reusable behavior.
//!
//! Services in this module are infrastructure-free: they take values in,
//! produce values out, and contain no async / IO / DB code. Wiring them
//! into the database, HTTP handlers, or import pipelines happens in the
//! `finima-api` and `finima-ingest` crates.

pub mod sign_normalizer;
