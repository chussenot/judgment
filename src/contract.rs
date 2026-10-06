//! The published contract the crate is tested against, for a consumer that
//! validates its own TypeSafe traffic against the same document.
//!
//! [`OPENAPI_DOCUMENT`] is the text of `tests/fixtures/typesafe-openapi.json`,
//! the copy of <https://api.typesafe.ai/openapi.json> that `tests/contract.rs`
//! checks every request shape, `Fake` response and committed recording
//! against, and that only `tests/openapi_drift.rs` refreshes. It is behind
//! the `openapi` feature because it is static data a client has no use for
//! at run time; an application turns the feature on in its dev-dependencies
//! and parses the text with `serde_json`, so its checks and the crate's
//! cannot drift apart. The document is what TypeSafe publishes, not what the
//! crate enforces; where the crate is stricter (the API reference page's
//! limits) or more tolerant (a missing `usage`) is pinned in `tests/contract.rs`.

/// The vendored TypeSafe System One OpenAPI document, as JSON text.
pub const OPENAPI_DOCUMENT: &str = include_str!("../tests/fixtures/typesafe-openapi.json");
