//! The schemas generated from the `.jud` types (`jud::schema`, feature
//! `schema`) against the hand-written ones in `schemas/jud/`: the same
//! fields, the same required fields, closed where the committed schema is
//! closed. The committed schemas are the specification; this fails when a
//! type and its schema stop describing the same fields.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

use std::collections::BTreeSet;

use serde_json::Value;

const SCHEMAS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/schemas/jud");

fn committed(kind: &str) -> Value {
    let text = std::fs::read_to_string(format!("{SCHEMAS}/{kind}.schema.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// What a shape says about its object: field names, required names, whether
/// an unknown field is refused. A type that tolerates one (`BoxEdges` reads
/// any four numbers, its range checked elsewhere) says nothing, while the
/// format refuses it, so only a closed type must meet a closed schema.
#[derive(Debug, PartialEq)]
struct Shape {
    fields: BTreeSet<String>,
    required: BTreeSet<String>,
    closed: bool,
}

fn names(value: Option<&Value>) -> BTreeSet<String> {
    let strings = |v: &Value| v.as_str().map(str::to_owned);
    match value {
        Some(Value::Array(items)) => items.iter().filter_map(strings).collect(),
        Some(Value::Object(map)) => map.keys().cloned().collect(),
        _ => BTreeSet::new(),
    }
}

fn shape(schema: &Value) -> Shape {
    Shape {
        fields: names(schema.get("properties")),
        required: names(schema.get("required")),
        closed: schema.get("additionalProperties") == Some(&Value::Bool(false)),
    }
}

fn generated(name: &str) -> Shape {
    let defs = judgment::jud::schema::definitions();
    shape(
        defs.get(name)
            .unwrap_or_else(|| panic!("no generated {name}")),
    )
}

fn pointer<'a>(schema: &'a Value, path: &str) -> &'a Value {
    schema
        .pointer(path)
        .unwrap_or_else(|| panic!("no {path} in the committed schema"))
}

#[test]
fn each_generated_shape_matches_the_committed_schema() {
    let common = committed("common");
    let rubric = committed("rubric");
    let cases = committed("cases");
    let gate = pointer(
        &rubric,
        "/properties/spec/properties/policy/additionalProperties",
    );
    let expect = pointer(
        &cases,
        "/properties/spec/properties/cases/items/properties/expect/additionalProperties/oneOf",
    );

    // (generated type, the committed schema it must agree with)
    let pairs: [(&str, &Value); 9] = [
        ("Metadata", pointer(&common, "/$defs/metadata")),
        ("AssetSource", pointer(&common, "/$defs/asset")),
        ("BoxEdges", pointer(&common, "/$defs/boxEdges")),
        ("Gate", gate),
        ("Band", pointer(gate, "/properties/bands/items")),
        (
            "Tuning",
            pointer(&rubric, "/properties/spec/properties/tuning"),
        ),
        (
            "Case",
            pointer(&cases, "/properties/spec/properties/cases/items"),
        ),
        ("FromTurn", &expect[3]),
        ("BoxLabel", &expect[4]),
    ];
    let mut disagreements = Vec::new();
    for (name, schema) in pairs {
        let (generated, committed) = (generated(name), shape(schema));
        let agree = generated.fields == committed.fields
            && generated.required == committed.required
            && (!generated.closed || committed.closed);
        if !agree {
            disagreements.push(format!(
                "{name}:\n  generated {generated:?}\n  committed {committed:?}"
            ));
        }
    }
    assert!(disagreements.is_empty(), "{}", disagreements.join("\n"));
}
