//! JSON Schema for the `.jud` types whose Rust shape is the file's shape,
//! generated from the types by schemars (feature `schema`).
//!
//! These are not the published schemas. `schemas/jud/` stays the
//! specification other tools read, hand-written because what it says is more
//! than serde can derive: an `apiVersion` that decides whether `minor` is
//! required, name patterns, the question shapes. This module is the other
//! direction, a drift check: `tests/jud_schema.rs` holds each generated
//! shape's fields, required fields and closedness against the committed
//! schema, so a field added to a type and forgotten there fails a test.
//! Rubric, Cases and Recording themselves are read through wire structs and
//! are not covered; their parts below are.

use schemars::JsonSchema;
use serde_json::{Map, Value};

use super::{AssetSource, Band, BoxLabel, Case, FromTurn, Gate, Metadata, Tuning};
use crate::answer::BoxEdges;

/// One field per shape, so one schema holds them all under `$defs`.
#[derive(JsonSchema)]
#[allow(dead_code)]
struct Shapes {
    metadata: Metadata,
    asset: AssetSource,
    box_edges: BoxEdges,
    gate: Gate,
    band: Band,
    tuning: Tuning,
    case: Case,
    box_label: BoxLabel,
    from_turn: FromTurn,
}

/// The generated schema of each shape, by type name (`Metadata`,
/// `AssetSource`, `BoxEdges`, `Gate`, `Band`, `Tuning`, `Case`, `BoxLabel`,
/// `FromTurn`; a type they refer to, `Expect` or `LevelRef`, is there too).
#[must_use]
pub fn definitions() -> Map<String, Value> {
    let mut root = serde_json::to_value(schemars::schema_for!(Shapes)).unwrap_or_default();
    match root.get_mut("$defs").map(Value::take) {
        Some(Value::Object(defs)) => defs,
        _ => Map::new(),
    }
}
